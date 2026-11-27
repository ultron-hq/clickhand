//! First run: move to Applications (macOS), show the shortcut, connect the AI agents with
//! the bundled Clickhand, open the privacy settings it needs, and start at login.
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;

use crate::App;

#[derive(Serialize)]
pub struct Onboard {
    os: &'static str,
    /// macOS app bundle that isn't in /Applications yet
    needs_move: bool,
    shortcut: String,
    claude: bool,
    codex: bool,
    hands: bool,
    autostart: bool,
}

fn flag() -> PathBuf {
    ultron_core::paths::data_dir().join("desktop-welcome-done")
}

pub fn first_run() -> bool {
    !flag().exists()
}

pub fn which(bin: &str) -> Option<PathBuf> {
    let exts: Vec<&str> = if cfg!(windows) { vec![".exe", ".cmd", ""] } else { vec![""] };
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|d| exts.iter().map(|e| d.join(format!("{bin}{e}"))).find(|p| p.is_file()))
}

/// The Clickhand bundled next to this app (a sidecar), else one on PATH.
pub fn hands() -> Option<PathBuf> {
    let name = if cfg!(windows) { "clickhand-mcp.exe" } else { "clickhand-mcp" };
    let beside = std::env::current_exe().ok()?.parent()?.join(name);
    if std::fs::metadata(&beside).is_ok_and(|m| m.len() > 0) {
        return Some(beside);
    }
    which("clickhand")
}

/// `/…/Clickhand.app` when running from a macOS app bundle.
fn bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let app = exe.parent()?.parent()?.parent()?;
    (app.extension().is_some_and(|e| e == "app")).then(|| app.to_path_buf())
}

fn needs_move() -> bool {
    cfg!(target_os = "macos") && bundle().is_some_and(|b| !b.starts_with("/Applications"))
}

#[tauri::command]
pub fn onboard_state(app: AppHandle, st: State<App>) -> Onboard {
    Onboard {
        os: if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(windows) {
            "windows"
        } else {
            "linux"
        },
        needs_move: needs_move(),
        shortcut: st.shortcut.lock().unwrap().clone(),
        claude: which("claude").is_some(),
        codex: which("codex").is_some(),
        hands: hands().is_some(),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
    }
}

/// Copy the app into /Applications (asking for an admin password only if needed),
/// clear the download quarantine, open the copy and quit this one.
#[tauri::command]
pub fn move_to_applications(app: AppHandle) -> Result<(), String> {
    let src = bundle().ok_or("Clickhand isn't running from an app bundle.")?;
    let dest = Path::new("/Applications").join(src.file_name().ok_or("bad bundle path")?);
    let (s, d) = (src.to_string_lossy().to_string(), dest.to_string_lossy().to_string());
    let copied = Command::new("ditto").args([&s, &d]).status().is_ok_and(|st| st.success());
    if !copied {
        let script = format!(
            "do shell script \"ditto '{}' '{}'\" with administrator privileges",
            s.replace('\'', "'\\''"),
            d.replace('\'', "'\\''")
        );
        let ok = Command::new("osascript")
            .args(["-e", &script])
            .status()
            .is_ok_and(|st| st.success());
        if !ok {
            return Err("Couldn't copy Clickhand to Applications. Drag it there from the disk image instead.".into());
        }
    }
    let _ = Command::new("xattr").args(["-dr", "com.apple.quarantine", &d]).status();
    Command::new("open").args(["-n", &d]).spawn().map_err(|e| e.to_string())?;
    app.exit(0);
    Ok(())
}

/// Run the bundled `clickhand setup --yes`: connects every agent it finds and
/// installs the skills. Returns its report.
#[tauri::command]
pub async fn connect_agents() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let bin = hands().ok_or("Clickhand isn't bundled with this build.")?;
        let out = Command::new(bin)
            .args(["setup", "--yes"])
            .env("NO_COLOR", "1")
            .output()
            .map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&out.stdout);
        // keep the agent and check lines; drop the banner and the "try it" footer
        let lines: Vec<&str> = text
            .lines()
            .map(str::trim)
            .filter(|l| l.starts_with('✓') || l.starts_with('✗') || l.starts_with('!'))
            .collect();
        Ok(if lines.is_empty() {
            text.trim().to_string()
        } else {
            lines.join("\n")
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Open the macOS privacy pane Hands needs: "screen" or "accessibility".
#[tauri::command]
pub fn open_settings(pane: String) {
    if cfg!(target_os = "macos") {
        let anchor = if pane == "screen" {
            "Privacy_ScreenCapture"
        } else {
            "Privacy_Accessibility"
        };
        let _ = Command::new("open")
            .arg(format!("x-apple.systempreferences:com.apple.preference.security?{anchor}"))
            .spawn();
    }
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, on: bool) -> Result<bool, String> {
    let a = app.autolaunch();
    if on { a.enable() } else { a.disable() }.map_err(|e| e.to_string())?;
    a.is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn finish_onboarding(app: AppHandle) {
    let f = flag();
    if let Some(dir) = f.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&f, "done\n");
    if let Some(w) = app.get_webview_window("welcome") {
        let _ = w.hide();
    }
}

pub fn show(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("welcome") {
        let _ = w.center();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// macOS privacy permissions, checked and requested for this app. Hands runs as this app's
/// broker, so these two grants cover every AI agent.
#[cfg(target_os = "macos")]
mod tcc {
    use std::ffi::c_void;
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGRequestScreenCaptureAccess() -> bool;
    }
    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> bool;
        fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
        static kAXTrustedCheckOptionPrompt: *const c_void;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        static kCFBooleanTrue: *const c_void;
        static kCFTypeDictionaryKeyCallBacks: c_void;
        static kCFTypeDictionaryValueCallBacks: c_void;
        fn CFDictionaryCreate(
            alloc: *const c_void,
            keys: *const *const c_void,
            values: *const *const c_void,
            n: isize,
            kcb: *const c_void,
            vcb: *const c_void,
        ) -> *const c_void;
        fn CFRelease(cf: *const c_void);
    }
    pub fn screen() -> bool {
        unsafe { CGPreflightScreenCaptureAccess() }
    }
    pub fn accessibility() -> bool {
        unsafe { AXIsProcessTrusted() }
    }
    pub fn ask_screen() {
        unsafe {
            CGRequestScreenCaptureAccess();
        }
    }
    /// Shows macOS's own prompt, which also adds Clickhand to the Accessibility list.
    pub fn ask_accessibility() {
        unsafe {
            let keys = [kAXTrustedCheckOptionPrompt];
            let values = [kCFBooleanTrue];
            let d = CFDictionaryCreate(
                std::ptr::null(),
                keys.as_ptr(),
                values.as_ptr(),
                1,
                &raw const kCFTypeDictionaryKeyCallBacks,
                &raw const kCFTypeDictionaryValueCallBacks,
            );
            AXIsProcessTrustedWithOptions(d);
            if !d.is_null() {
                CFRelease(d);
            }
        }
    }
}

#[derive(Serialize)]
pub struct Permissions {
    screen: bool,
    accessibility: bool,
}

#[tauri::command]
pub fn permissions() -> Permissions {
    #[cfg(target_os = "macos")]
    return Permissions {
        screen: tcc::screen(),
        accessibility: tcc::accessibility(),
    };
    #[cfg(not(target_os = "macos"))]
    Permissions {
        screen: true,
        accessibility: true,
    }
}

/// Trigger macOS's prompt for "screen" or "accessibility", then open its settings pane.
#[tauri::command]
pub fn request_permission(pane: String) {
    #[cfg(target_os = "macos")]
    if pane == "screen" {
        tcc::ask_screen();
    } else {
        tcc::ask_accessibility();
    }
    open_settings(pane);
}

/// Screen Recording only takes effect after a restart.
#[tauri::command]
pub fn relaunch(app: AppHandle) {
    app.restart();
}

/// Start the bundled Hands as this app's broker, so agent-started copies relay to it and
/// macOS checks Clickhand's permissions, not Terminal's or Cursor's.
#[cfg(target_os = "macos")]
pub fn start_broker() -> Option<std::process::Child> {
    let bin = hands()?;
    Command::new(bin)
        .args(["broker", "--parent", &std::process::id().to_string()])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()
}
