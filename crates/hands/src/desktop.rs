//! Windows, apps and URLs. Linux uses xdotool for windows, as the original does;
//! Windows and macOS list windows through xcap; focus works on Linux and Windows. Apps come
//! from .desktop files, /Applications or the Start Menu.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::HandsError;

/// Run a command with a timeout; its stdout, trimmed.
pub fn run(cmd: &[&str], timeout: f64, check: bool) -> Result<String, HandsError> {
    let mut child = Command::new(cmd[0])
        .args(&cmd[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                HandsError::new(format!("`{}` is not installed.", cmd[0]))
            } else {
                HandsError::new(e.to_string())
            }
        })?;
    let deadline = Instant::now() + Duration::from_secs_f64(timeout);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                return Err(HandsError::new(format!(
                    "`{}` timed out after {timeout}s (is the screen locked or the window gone?).",
                    cmd[0]
                )));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(e) => return Err(HandsError::new(e.to_string())),
        }
    }
    let out = child.wait_with_output().map_err(|e| HandsError::new(e.to_string()))?;
    if check && !out.status.success() {
        let text = if out.stderr.is_empty() { &out.stdout } else { &out.stderr };
        let text = String::from_utf8_lossy(text);
        let why = text.trim().lines().last().unwrap_or("no output").to_string();
        let head = cmd.iter().take(3).copied().collect::<Vec<_>>().join(" ");
        return Err(HandsError::new(format!(
            "`{head}` failed (exit {}): {why}",
            out.status.code().unwrap_or(-1)
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn spawn_detached(cmd: &[&str]) -> Result<(), HandsError> {
    Command::new(cmd[0])
        .args(&cmd[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| HandsError::new(format!("Could not start `{}`: {e}", cmd[0])))
}

// ── windows ──

#[cfg(target_os = "linux")]
pub fn windows(limit: usize) -> Vec<Value> {
    let ids = run(&["xdotool", "search", "--onlyvisible", "--name", "."], 8.0, false).unwrap_or_default();
    let active = run(&["xdotool", "getactivewindow"], 8.0, false).unwrap_or_default();
    let mut out = Vec::new();
    for wid in ids.split_whitespace() {
        let name = run(&["xdotool", "getwindowname", wid], 8.0, false).unwrap_or_default();
        let geo = run(&["xdotool", "getwindowgeometry", "--shell", wid], 8.0, false).unwrap_or_default();
        if name.is_empty() || geo.is_empty() || name == "openhud overlay" {
            continue;
        }
        let g: BTreeMap<&str, i64> = geo
            .split_whitespace()
            .filter_map(|kv| kv.split_once('='))
            .filter_map(|(k, v)| Some((k, v.parse().ok()?)))
            .collect();
        let (w, h) = (g.get("WIDTH").copied().unwrap_or(0), g.get("HEIGHT").copied().unwrap_or(0));
        if w < 80 || h < 40 {
            continue;
        }
        let pid = run(&["xdotool", "getwindowpid", wid], 8.0, false).unwrap_or_default();
        out.push(json!({
            "id": wid.parse::<i64>().unwrap_or(0), "title": name, "pid": pid.parse::<i64>().ok(),
            "x": g.get("X").copied().unwrap_or(0), "y": g.get("Y").copied().unwrap_or(0), "w": w, "h": h,
            "active": wid == active,
        }));
    }
    out.truncate(limit);
    out
}

#[cfg(not(target_os = "linux"))]
pub fn windows(limit: usize) -> Vec<Value> {
    let Ok(all) = xcap::Window::all() else { return vec![] };
    all.into_iter()
        .filter(|w| !w.is_minimized().unwrap_or(false))
        .filter(|w| w.width().unwrap_or(0) >= 80 && w.height().unwrap_or(0) >= 40)
        .filter_map(|w| {
            let title = w.title().ok().filter(|t| !t.is_empty())?;
            Some(json!({
                "id": w.id().unwrap_or(0), "title": title, "pid": w.pid().ok(),
                "x": w.x().unwrap_or(0), "y": w.y().unwrap_or(0), "w": w.width().unwrap_or(0), "h": w.height().unwrap_or(0),
                "active": w.is_focused().unwrap_or(false),
            }))
        })
        .take(limit)
        .collect()
}

/// Bring a window to the front and verify it really took focus.
#[cfg(target_os = "linux")]
pub fn focus(window_id: i64) -> Result<(), HandsError> {
    let wid = window_id.to_string();
    if run(&["xdotool", "getwindowname", &wid], 8.0, false)?.is_empty() {
        return Err(HandsError::new(format!(
            "Window {window_id} no longer exists. Call list_windows for current ids."
        )));
    }
    let _ = run(&["xdotool", "windowactivate", &wid], 8.0, false);
    std::thread::sleep(Duration::from_millis(150));
    if run(&["xdotool", "getactivewindow"], 8.0, false).unwrap_or_default() != wid {
        let _ = run(&["xdotool", "windowraise", &wid], 8.0, false);
        let _ = run(&["xdotool", "windowfocus", &wid], 8.0, false);
        std::thread::sleep(Duration::from_millis(150));
    }
    let now = run(&["xdotool", "getactivewindow"], 8.0, false).unwrap_or_default();
    if now != wid {
        let shown = if now.is_empty() { "none".to_string() } else { now };
        return Err(HandsError::new(format!(
            "Window {window_id} would not take focus (active is {shown}). It may be minimized or on another workspace; try alt+tab or click it."
        )));
    }
    Ok(())
}

/// The window with keyboard focus, as a `list_windows` id. None where it can't be told
/// (macOS for now), in which case typing isn't guarded against focus changes.
#[cfg(target_os = "linux")]
pub fn active_window() -> Option<i64> {
    run(&["xdotool", "getactivewindow"], 2.0, false).ok()?.trim().parse().ok()
}

#[cfg(windows)]
pub fn active_window() -> Option<i64> {
    let h = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
    // list_windows (xcap) reports a window's id as its handle in 32 bits
    (!h.is_null()).then(|| h as usize as u32 as i64)
}

#[cfg(target_os = "macos")]
pub fn active_window() -> Option<i64> {
    None
}

/// Windows only lets the app in front hand focus to another window. Restore the window
/// if it's minimized, borrow the foreground thread's input state, and if that isn't
/// enough, press and release Alt (the documented way to unlock it); then verify.
#[cfg(windows)]
pub fn focus(window_id: i64) -> Result<(), HandsError> {
    use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{keybd_event, KEYEVENTF_KEYUP, VK_MENU};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, IsIconic, IsWindow, SetForegroundWindow, ShowWindow, SW_RESTORE,
    };
    let hwnd = window_id as usize as windows_sys::Win32::Foundation::HWND;
    let want = window_id as u32 as i64;
    if unsafe { IsWindow(hwnd) } == 0 {
        return Err(HandsError::new(format!(
            "Window {window_id} no longer exists. Call list_windows for current ids."
        )));
    }
    let try_focus = |alt: bool| unsafe {
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        let fg = GetForegroundWindow();
        let fg_thread = GetWindowThreadProcessId(fg, std::ptr::null_mut());
        let me = GetCurrentThreadId();
        let attached = fg_thread != 0 && fg_thread != me && AttachThreadInput(me, fg_thread, 1) != 0;
        if alt {
            keybd_event(VK_MENU as u8, 0, 0, 0);
            keybd_event(VK_MENU as u8, 0, KEYEVENTF_KEYUP, 0);
        }
        BringWindowToTop(hwnd);
        SetForegroundWindow(hwnd);
        if attached {
            AttachThreadInput(me, fg_thread, 0);
        }
    };
    for alt in [false, true] {
        try_focus(alt);
        std::thread::sleep(Duration::from_millis(150));
        if active_window() == Some(want) {
            return Ok(());
        }
    }
    let now = active_window().map(|w| w.to_string()).unwrap_or_else(|| "none".into());
    Err(HandsError::new(format!(
        "Window {window_id} would not come to the front (active is {now}); Windows can block this while another app is busy or the screen is locked. Click the window's title bar instead, then check with a screenshot."
    )))
}

#[cfg(target_os = "macos")]
pub fn focus(_window_id: i64) -> Result<(), HandsError> {
    Err(HandsError::new(
        "focus_window isn't supported on macOS yet: click the window (its title bar is safest) or use cmd+tab, then take a screenshot to check.",
    ))
}

// ── apps ──

/// Installed GUI apps: display name → how to launch it.
pub fn apps() -> BTreeMap<String, PathBuf> {
    let mut apps = BTreeMap::new();
    if cfg!(target_os = "linux") {
        let home = dirs::home_dir().unwrap_or_default();
        let dirs = [
            PathBuf::from("/usr/share/applications"),
            PathBuf::from("/usr/local/share/applications"),
            home.join(".local/share/applications"),
            PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        ];
        for dir in dirs {
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            for e in entries.flatten() {
                let p = e.path();
                if p.extension().is_none_or(|x| x != "desktop") {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&p) else { continue };
                let line = |k: &str| text.lines().find_map(|l| l.strip_prefix(k).map(str::trim).map(str::to_string));
                if text.lines().any(|l| l == "NoDisplay=true" || l == "Hidden=true") || text.lines().any(|l| l == "Terminal=true") {
                    continue;
                }
                if let Some(name) = line("Name=") {
                    apps.insert(name, p);
                }
            }
        }
    } else if cfg!(target_os = "macos") {
        let home = dirs::home_dir().unwrap_or_default();
        for dir in [
            PathBuf::from("/Applications"),
            PathBuf::from("/System/Applications"),
            home.join("Applications"),
        ] {
            for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
                let p = e.path();
                if p.extension().is_some_and(|x| x == "app") {
                    if let Some(stem) = p.file_stem() {
                        apps.insert(stem.to_string_lossy().into_owned(), p);
                    }
                }
            }
        }
    } else {
        let mut dirs: Vec<PathBuf> = Vec::new();
        for var in ["ProgramData", "APPDATA"] {
            if let Some(base) = std::env::var_os(var) {
                dirs.push(PathBuf::from(base).join("Microsoft/Windows/Start Menu/Programs"));
            }
        }
        fn walk(dir: &Path, apps: &mut BTreeMap<String, PathBuf>, depth: u8) {
            for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                let p = e.path();
                if p.is_dir() && depth < 4 {
                    walk(&p, apps, depth + 1);
                } else if p.extension().is_some_and(|x| x == "lnk") {
                    if let Some(stem) = p.file_stem() {
                        apps.insert(stem.to_string_lossy().into_owned(), p.clone());
                    }
                }
            }
        }
        for d in dirs {
            walk(&d, &mut apps, 0);
        }
    }
    apps
}

pub fn launch(path: &Path) -> Result<(), HandsError> {
    let p = path.to_string_lossy().to_string();
    if cfg!(target_os = "linux") {
        let id = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        spawn_detached(&["gtk-launch", &id])
    } else if cfg!(target_os = "macos") {
        spawn_detached(&["open", "-a", &p])
    } else {
        spawn_detached(&["cmd", "/C", "start", "", &p])
    }
}

/// Only http(s) URLs with a host.
pub fn check_url(url: &str) -> Result<(), HandsError> {
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"));
    let host = rest.map(|r| {
        r.split(['/', '?', '#'])
            .next()
            .unwrap_or("")
            .rsplit('@')
            .next()
            .unwrap_or("")
            .split(':')
            .next()
            .unwrap_or("")
    });
    match host {
        Some(h) if !h.is_empty() => Ok(()),
        _ => Err(HandsError::new("Only http:// or https:// URLs can be opened.")),
    }
}

pub fn open_url(url: &str) -> Result<(), HandsError> {
    check_url(url)?;
    if cfg!(target_os = "linux") {
        spawn_detached(&["xdg-open", url])
    } else if cfg!(target_os = "macos") {
        spawn_detached(&["open", url])
    } else {
        spawn_detached(&["cmd", "/C", "start", "", url])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_http_urls_with_a_host() {
        for ok in ["https://x.com/compose/post", "http://localhost:3000", "https://user@site.com/a?b#c"] {
            assert!(check_url(ok).is_ok(), "{ok}");
        }
        for bad in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "https://",
            "ftp://site.com",
            "x.com",
            "https:///path",
        ] {
            assert!(check_url(bad).is_err(), "{bad}");
        }
    }
}
