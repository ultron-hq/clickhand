//! OpenHUD, the desktop app: a flip-clock island at the top of the screen, a prompt bar on
//! Ctrl+Space, and a tray icon. Timers are handled here and logged to the sessions
//! database (so reports keep working); anything else goes to Claude Code or Codex,
//! which can use the computer through OpenHUD.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod intent;
mod onboard;

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use ultron_core::sessions::Tracker;

use intent::Intent;

fn now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

/// What the island shows. Times are Unix seconds.
#[derive(Default, Clone, Serialize)]
struct Clock {
    /// "" (just the time), "timer" or "stopwatch"
    kind: String,
    label: String,
    started: f64,
    planned: Option<f64>,
    paused_at: Option<f64>,
    paused_total: f64,
}

impl Clock {
    fn elapsed(&self) -> f64 {
        let end = self.paused_at.unwrap_or_else(now);
        (end - self.started - self.paused_total).max(0.0)
    }
}

type SessionTracker = Tracker<fn() -> f64, fn() -> f64>;

pub struct App {
    clock: Mutex<Clock>,
    tracker: Mutex<SessionTracker>,
    pub shortcut: Mutex<String>,
    request: Mutex<Option<Request>>,
}

#[tauri::command]
fn shortcut(app: State<App>) -> String {
    app.shortcut.lock().unwrap().clone()
}

#[tauri::command]
fn clock(app: State<App>) -> Clock {
    let mut c = app.clock.lock().unwrap();
    // a finished timer goes back to the time on its own
    if c.kind == "timer" && c.paused_at.is_none() && c.planned.is_some_and(|p| c.elapsed() >= p) {
        app.tracker.lock().unwrap().finish("completed");
        *c = Clock::default();
    }
    c.clone()
}

/// The request the prompt bar shows: what was asked and where it stands. It lives here,
/// not in the bar, so hiding and reopening the bar never loses it.
#[derive(Clone, Serialize, Default)]
struct Request {
    id: u64,
    prompt: String,
    /// "running", "done", "error" or "cancelled"
    status: String,
    output: String,
    #[serde(skip)]
    pid: Option<u32>,
}

static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn publish(handle: &AppHandle, r: &Request) {
    let _ = handle.emit("request", r.clone());
}

#[tauri::command]
fn request(app: State<'_, App>) -> Option<Request> {
    app.request.lock().unwrap().clone()
}

/// Stop the running request: its process (and everything it started) is killed, and a
/// late answer can no longer replace what the bar shows.
#[tauri::command]
fn cancel(app: State<'_, App>, handle: AppHandle) -> Option<Request> {
    let mut slot = app.request.lock().unwrap();
    let r = slot.as_mut()?;
    if r.status == "running" {
        if let Some(pid) = r.pid.take() {
            agent::kill_tree(pid);
        }
        r.status = "cancelled".into();
        r.output = "Cancelled. It stopped before answering.".into();
        publish(&handle, r);
    }
    Some(r.clone())
}

#[tauri::command]
fn ask(text: String, app: State<'_, App>, handle: AppHandle) -> Request {
    let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    // A new request replaces the old one; stop it if it's still working.
    if let Some(old) = app.request.lock().unwrap().as_mut() {
        if old.status == "running" {
            if let Some(pid) = old.pid.take() {
                agent::kill_tree(pid);
            }
            old.status = "cancelled".into();
        }
    }
    let mut r = Request {
        id,
        prompt: text.clone(),
        status: "running".into(),
        ..Request::default()
    };
    if matches!(intent::parse(&text), Intent::Agent) {
        *app.request.lock().unwrap() = Some(r.clone());
        std::thread::spawn(move || {
            let state = handle.state::<App>();
            let result = agent::run(&text, id, |pid| {
                if let Some(cur) = state.request.lock().unwrap().as_mut().filter(|c| c.id == id) {
                    cur.pid = Some(pid);
                }
            });
            let mut slot = state.request.lock().unwrap();
            // Only the current, still-running request may take the answer.
            let Some(cur) = slot.as_mut().filter(|c| c.id == id && c.status == "running") else {
                return;
            };
            cur.pid = None;
            match result {
                Ok(out) => {
                    cur.status = "done".into();
                    cur.output = out;
                }
                Err(e) => {
                    cur.status = "error".into();
                    cur.output = e;
                }
            }
            publish(&handle, cur);
        });
        return r;
    }
    match local(&text, &app) {
        Ok(out) => {
            r.status = "done".into();
            r.output = out;
        }
        Err(e) => {
            r.status = "error".into();
            r.output = e;
        }
    }
    *app.request.lock().unwrap() = Some(r.clone());
    r
}

/// Requests answered on the spot: timers, the stopwatch, pause, resume and stop.
fn local(text: &str, app: &App) -> Result<String, String> {
    match intent::parse(text) {
        Intent::Timer { seconds, label } => {
            let mut t = app.tracker.lock().unwrap();
            if t.running() {
                t.finish("stopped");
            }
            t.start("timer", label.as_deref(), Some(seconds));
            *app.clock.lock().unwrap() = Clock {
                kind: "timer".into(),
                label: label.clone().unwrap_or_default(),
                started: now(),
                planned: Some(seconds),
                ..Clock::default()
            };
            Ok(format!(
                "Timer started: {}{}",
                intent::human(seconds),
                label.map(|l| format!(" for {l}")).unwrap_or_default()
            ))
        }
        Intent::Stopwatch { label } => {
            let mut t = app.tracker.lock().unwrap();
            if t.running() {
                t.finish("stopped");
            }
            t.start("stopwatch", label.as_deref(), None);
            *app.clock.lock().unwrap() = Clock {
                kind: "stopwatch".into(),
                label: label.unwrap_or_default(),
                started: now(),
                ..Clock::default()
            };
            Ok("Stopwatch started".into())
        }
        Intent::Pause => {
            let mut c = app.clock.lock().unwrap();
            if c.kind.is_empty() || c.paused_at.is_some() {
                return Ok("Nothing is running".into());
            }
            c.paused_at = Some(now());
            app.tracker.lock().unwrap().pause();
            Ok("Paused".into())
        }
        Intent::Resume => {
            let mut c = app.clock.lock().unwrap();
            let Some(at) = c.paused_at.take() else {
                return Ok("Nothing is paused".into());
            };
            c.paused_total += now() - at;
            app.tracker.lock().unwrap().resume();
            Ok("Resumed".into())
        }
        Intent::Stop => {
            let mut c = app.clock.lock().unwrap();
            if c.kind.is_empty() {
                return Ok("Nothing is running".into());
            }
            let took = intent::human(c.elapsed());
            app.tracker.lock().unwrap().finish("stopped");
            *c = Clock::default();
            Ok(format!("Stopped after {took}"))
        }
        Intent::Agent => Err("That needs Claude Code or Codex.".into()),
    }
}

/// "macos", "windows" or "linux", so each window can follow its OS's conventions.
#[tauri::command]
fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(windows) {
        "windows"
    } else {
        "linux"
    }
}

/// Which AI answers what the bar can't: shown in the bar's footer.
#[tauri::command]
fn agent_name() -> &'static str {
    agent::name()
}

/// The bar's window follows its content, so no invisible part of it blocks clicks.
#[tauri::command]
fn fit_bar(app: AppHandle, height: f64) {
    if let Some(w) = app.get_webview_window("bar") {
        let _ = w.set_size(tauri::LogicalSize::new(BAR_W, height.clamp(64.0, 520.0)));
    }
}

const BAR_W: f64 = 680.0;

#[tauri::command]
fn hide_bar(app: AppHandle) {
    if let Some(w) = app.get_webview_window("bar") {
        let _ = w.hide();
    }
}

#[tauri::command]
fn toggle_bar(app: AppHandle) {
    toggle(&app);
}

fn toggle(app: &AppHandle) {
    let Some(w) = app.get_webview_window("bar") else { return };
    if w.is_visible().unwrap_or(false) {
        let _ = w.hide();
    } else {
        place(app, "bar", 180.0);
        let _ = w.show();
        let _ = w.set_focus();
        let _ = w.emit("bar-open", ());
        // the window manager may move it on show; put it back in the middle
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(60));
            place(&app, "bar", 180.0);
        });
    }
}

/// Place a window horizontally centred on the primary monitor, `top` logical px down.
fn place(app: &AppHandle, label: &str, top: f64) {
    let Some(w) = app.get_webview_window(label) else { return };
    // Some window managers grow small undecorated windows; the island must stay its own
    // size, or its invisible part would block clicks on whatever is under it.
    if label == "island" {
        let _ = w.set_size(tauri::LogicalSize::new(236.0, 52.0));
        // GTK gives the webview a 200 px minimum height; ask for the island's own size.
        #[cfg(target_os = "linux")]
        {
            use gtk::prelude::{GtkWindowExt, WidgetExt};
            if let Ok(gw) = w.gtk_window() {
                gw.set_size_request(236, 52);
                gw.resize(236, 52);
            }
            if let Ok(vbox) = w.default_vbox() {
                vbox.set_size_request(236, 52);
            }
        }
    }
    let Ok(Some(m)) = w.primary_monitor() else { return };
    let (size, pos, scale) = (m.size(), m.position(), m.scale_factor());
    let Ok(ws) = w.outer_size() else { return };
    let x = pos.x + (size.width as i32 - ws.width as i32) / 2;
    let y = pos.y + (top * scale) as i32;
    let _ = w.set_position(PhysicalPosition::new(x, y));
}

mod agent {
    use std::io::Read;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};

    fn which(bin: &str) -> Option<PathBuf> {
        let exts: Vec<&str> = if cfg!(windows) { vec![".exe", ".cmd", ""] } else { vec![""] };
        let path = std::env::var_os("PATH")?;
        std::env::split_paths(&path).find_map(|d| exts.iter().map(|e| d.join(format!("{bin}{e}"))).find(|p| p.is_file()))
    }

    pub fn name() -> &'static str {
        if which("claude").is_some() {
            "Claude Code"
        } else if which("codex").is_some() {
            "Codex"
        } else {
            ""
        }
    }

    const BRIEF: &str = "You are OpenHUD, a desktop assistant the person reached with Ctrl+Space. \
        Answer in one or two short sentences. If the request needs the computer, use the openhud tools.";

    /// A background command: no console window on Windows (the whole chain, so a
    /// `codex.cmd` wrapper and the node it starts stay hidden too), its own process
    /// group elsewhere so cancelling stops everything it started.
    pub fn hidden(bin: impl AsRef<std::ffi::OsStr>) -> Command {
        let mut c = Command::new(bin);
        c.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            c.creation_flags(CREATE_NO_WINDOW);
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            c.process_group(0);
        }
        c
    }

    /// Kill a request's process and everything it started.
    pub fn kill_tree(pid: u32) {
        #[cfg(windows)]
        let _ = hidden("taskkill").args(["/PID", &pid.to_string(), "/T", "/F"]).output();
        #[cfg(unix)]
        let _ = Command::new("kill").args(["-TERM", "--", &format!("-{pid}")]).output();
    }

    /// Hand the request to Claude Code, or Codex, with OpenHUD available. `started`
    /// gets the process id so the request can be cancelled.
    pub fn run(text: &str, id: u64, started: impl FnOnce(u32)) -> Result<String, String> {
        let prompt = format!("{BRIEF}\n\n{text}");
        let mut answer_file = None;
        let mut cmd = if let Some(claude) = which("claude") {
            let mut c = hidden(claude);
            c.args(["-p", &prompt, "--allowedTools", "mcp__openhud"]);
            c
        } else if let Some(codex) = which("codex") {
            let file = std::env::temp_dir().join(format!("openhud-answer-{}-{id}.txt", std::process::id()));
            let mut c = hidden(codex);
            c.args([
                "exec",
                "--skip-git-repo-check",
                "-c",
                "mcp_servers.openhud.default_tools_approval_mode=\"approve\"",
                "-o",
            ])
            .arg(&file)
            .arg(&prompt);
            answer_file = Some(file);
            c
        } else {
            return Err("Install Claude Code or Codex so OpenHUD can answer that (timers work without them).".into());
        };
        let mut child = cmd.spawn().map_err(|e| e.to_string())?;
        started(child.id());
        // Read both pipes while it runs, so a long answer can't fill one and stall it.
        let mut out_pipe = child.stdout.take();
        let reader = std::thread::spawn(move || {
            let mut s = String::new();
            if let Some(p) = out_pipe.as_mut() {
                let _ = p.read_to_string(&mut s);
            }
            s
        });
        let mut err = String::new();
        if let Some(mut p) = child.stderr.take() {
            let _ = p.read_to_string(&mut err);
        }
        let status = child.wait().map_err(|e| e.to_string())?;
        let stdout = reader.join().unwrap_or_default();
        if let Some(file) = answer_file {
            if let Ok(answer) = std::fs::read_to_string(&file) {
                let _ = std::fs::remove_file(&file);
                if !answer.trim().is_empty() {
                    return Ok(answer.trim().to_string());
                }
            }
        }
        let text = stdout.trim().to_string();
        if status.success() && !text.is_empty() {
            Ok(text)
        } else {
            Err(err
                .lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("The agent didn't answer.")
                .to_string())
        }
    }
}

/// "ctrl+alt+space" → "Ctrl+Alt+Space" (Windows, Linux) or "⌃⌥ Space" (macOS).
fn pretty(key: &str) -> String {
    let parts: Vec<&str> = key.split('+').collect();
    if cfg!(target_os = "macos") {
        let mut mods = String::new();
        let mut rest = Vec::new();
        for p in &parts {
            match p.to_lowercase().as_str() {
                "ctrl" | "control" => mods.push('⌃'),
                "alt" | "option" => mods.push('⌥'),
                "shift" => mods.push('⇧'),
                "cmd" | "super" | "command" | "meta" => mods.push('⌘'),
                other => rest.push(cap(other)),
            }
        }
        return format!("{mods} {}", rest.join(" ")).trim().to_string();
    }
    parts.iter().map(|p| cap(p)).collect::<Vec<_>>().join("+")
}

fn cap(p: &str) -> String {
    let mut c = p.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

/// The default shortcut. On a Mac Ctrl+Space switches the keyboard's input source, so
/// OpenHUD takes ⌥ Space there.
fn default_shortcut() -> &'static str {
    if cfg!(target_os = "macos") {
        "alt+space"
    } else {
        "ctrl+space"
    }
}

fn main() {
    let tracker: SessionTracker = Tracker::system(None);
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        toggle(app);
                    }
                })
                .build(),
        )
        .manage(App {
            clock: Mutex::new(Clock::default()),
            tracker: Mutex::new(tracker),
            shortcut: Mutex::new(String::new()),
            request: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            clock,
            ask,
            request,
            platform,
            agent_name,
            fit_bar,
            cancel,
            hide_bar,
            toggle_bar,
            shortcut,
            onboard::onboard_state,
            onboard::move_to_applications,
            onboard::connect_agents,
            onboard::open_settings,
            onboard::set_autostart,
            onboard::finish_onboarding,
            onboard::permissions,
            onboard::request_permission,
            onboard::relaunch
        ])
        .on_window_event(|w, e| {
            if w.label() == "bar" {
                if let WindowEvent::Focused(false) = e {
                    let _ = w.hide();
                }
            }
        })
        .setup(|app| {
            // Window managers place new windows themselves; move the island once it's up.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                for delay in [150, 600, 1500] {
                    std::thread::sleep(std::time::Duration::from_millis(delay));
                    place(&handle, "island", 8.0);
                }
            });

            // Ctrl+Space (⌥ Space on a Mac), or OPENHUD_SHORTCUT; fall back if something else holds it.
            let wanted = std::env::var("OPENHUD_SHORTCUT").unwrap_or_else(|_| default_shortcut().into());
            let mut bound = None;
            for key in [wanted.as_str(), "ctrl+alt+space"] {
                if let Ok(s) = key.parse::<Shortcut>() {
                    if app.global_shortcut().register(s).is_ok() {
                        bound = Some(key.to_string());
                        break;
                    }
                }
            }
            let hint = bound.clone().map(|k| pretty(&k)).unwrap_or_else(|| "click".into());
            *app.state::<App>().shortcut.lock().unwrap() = hint.clone();

            let ask_item = MenuItem::with_id(app, "bar", format!("Ask OpenHUD ({hint})"), true, None::<&str>)?;
            let welcome = MenuItem::with_id(app, "welcome", "Set up OpenHUD…", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit OpenHUD", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&ask_item, &welcome, &quit])?;
            let mut tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("OpenHUD")
                .on_menu_event(|app, ev| match ev.id().as_ref() {
                    "bar" => toggle(app),
                    "welcome" => onboard::show(app),
                    "quit" => app.exit(0),
                    _ => {}
                });
            // macOS: a monochrome menu-bar icon that follows light and dark menu bars, and the
            // menu on click. Windows and Linux: click the tray icon to ask, right-click for the menu.
            #[cfg(target_os = "macos")]
            {
                let img = tauri::image::Image::from_bytes(include_bytes!("../icons/tray-template.png"))?;
                tray = tray.icon(img).icon_as_template(true);
            }
            #[cfg(not(target_os = "macos"))]
            {
                if let Some(icon) = app.default_window_icon() {
                    tray = tray.icon(icon.clone());
                }
                tray = tray.show_menu_on_left_click(false).on_tray_icon_event(|tray, ev| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = ev
                    {
                        toggle(tray.app_handle());
                    }
                });
            }
            tray.build(app)?;

            // macOS: real frosted glass behind the bar and the island.
            #[cfg(target_os = "macos")]
            {
                use tauri::window::{Effect, EffectState, EffectsBuilder};
                for (label, radius) in [("bar", 16.0), ("island", 26.0)] {
                    if let Some(w) = app.get_webview_window(label) {
                        let _ = w.set_effects(
                            EffectsBuilder::new()
                                .effect(Effect::HudWindow)
                                .state(EffectState::Active)
                                .radius(radius)
                                .build(),
                        );
                    }
                }
            }
            // macOS: no Dock icon; OpenHUD lives in the menu bar and the island.
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                // Hands for every agent runs under OpenHUD, so one permission grant covers all.
                if let Some(child) = onboard::start_broker() {
                    app.manage(Mutex::new(Some(child)));
                }
            }
            if onboard::first_run() {
                onboard::show(app.handle());
            }
            // For screenshots and tests: OPENHUD_ASK runs a request at start, and
            // OPENHUD_SHOW=bar,welcome opens those windows.
            if let Ok(text) = std::env::var("OPENHUD_ASK") {
                let handle = app.handle().clone();
                let st = app.state::<App>();
                let _ = ask(text, st, handle);
            }
            if let Ok(show) = std::env::var("OPENHUD_SHOW") {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(1800));
                    for w in show.split(',') {
                        match w.trim() {
                            "bar" => toggle(&handle),
                            "welcome" => onboard::show(&handle),
                            _ => {}
                        }
                    }
                });
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("OpenHUD failed to start");
}
