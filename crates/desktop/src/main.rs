//! Ultron, the desktop app: a flip-clock island at the top of the screen, a prompt bar on
//! Ctrl+Space, and a tray icon. Timers are handled here and logged to the sessions
//! database (so reports keep working); anything else goes to Claude Code or Codex,
//! which can use the computer through Ultron Hands.
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

#[tauri::command]
async fn ask(text: String, app: State<'_, App>) -> Result<String, String> {
    match intent::parse(&text) {
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
        Intent::Agent => tauri::async_runtime::spawn_blocking(move || agent::run(&text))
            .await
            .map_err(|e| e.to_string())?,
    }
}

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
    let Ok(Some(m)) = w.primary_monitor() else { return };
    let (size, pos, scale) = (m.size(), m.position(), m.scale_factor());
    let Ok(ws) = w.outer_size() else { return };
    let x = pos.x + (size.width as i32 - ws.width as i32) / 2;
    let y = pos.y + (top * scale) as i32;
    let _ = w.set_position(PhysicalPosition::new(x, y));
}

mod agent {
    use std::path::PathBuf;
    use std::process::Command;

    fn which(bin: &str) -> Option<PathBuf> {
        let exts: Vec<&str> = if cfg!(windows) { vec![".exe", ".cmd", ""] } else { vec![""] };
        let path = std::env::var_os("PATH")?;
        std::env::split_paths(&path).find_map(|d| exts.iter().map(|e| d.join(format!("{bin}{e}"))).find(|p| p.is_file()))
    }

    const BRIEF: &str = "You are Ultron, a desktop assistant the person reached with Ctrl+Space. \
        Answer in one or two short sentences. If the request needs the computer, use the ultron-hands tools.";

    /// Hand the request to Claude Code, or Codex, with Ultron Hands available.
    pub fn run(text: &str) -> Result<String, String> {
        let prompt = format!("{BRIEF}\n\n{text}");
        let out = if let Some(claude) = which("claude") {
            Command::new(claude)
                .args(["-p", &prompt, "--allowedTools", "mcp__ultron-hands"])
                .output()
        } else if let Some(codex) = which("codex") {
            let file = std::env::temp_dir().join(format!("ultron-answer-{}.txt", std::process::id()));
            let r = Command::new(codex)
                .args([
                    "exec",
                    "--skip-git-repo-check",
                    "-c",
                    "mcp_servers.ultron-hands.default_tools_approval_mode=\"approve\"",
                    "-o",
                ])
                .arg(&file)
                .arg(&prompt)
                .output();
            if let Ok(answer) = std::fs::read_to_string(&file) {
                let _ = std::fs::remove_file(&file);
                return Ok(answer.trim().to_string());
            }
            r
        } else {
            return Err("Install Claude Code or Codex so Ultron can answer that (timers work without them).".into());
        };
        let out = out.map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if out.status.success() && !text.is_empty() {
            Ok(text)
        } else {
            let err = String::from_utf8_lossy(&out.stderr);
            Err(err
                .lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("The agent didn't answer.")
                .to_string())
        }
    }
}

/// "ctrl+alt+space" → "Ctrl+Alt+Space"
fn pretty(key: &str) -> String {
    key.split('+')
        .map(|p| {
            let mut c = p.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join("+")
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
        })
        .invoke_handler(tauri::generate_handler![
            clock,
            ask,
            hide_bar,
            toggle_bar,
            shortcut,
            onboard::onboard_state,
            onboard::move_to_applications,
            onboard::connect_agents,
            onboard::open_settings,
            onboard::set_autostart,
            onboard::finish_onboarding
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

            // Ctrl+Space, or ULTRON_SHORTCUT; fall back if something else holds it.
            let wanted = std::env::var("ULTRON_SHORTCUT").unwrap_or_else(|_| "ctrl+space".into());
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

            let ask_item = MenuItem::with_id(app, "bar", format!("Ask Ultron ({hint})"), true, None::<&str>)?;
            let welcome = MenuItem::with_id(app, "welcome", "Set up Ultron…", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Ultron", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&ask_item, &welcome, &quit])?;
            let mut tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("Ultron")
                .on_menu_event(|app, ev| match ev.id().as_ref() {
                    "bar" => toggle(app),
                    "welcome" => onboard::show(app),
                    "quit" => app.exit(0),
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            // macOS: no Dock icon; Ultron lives in the menu bar and the island.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            if onboard::first_run() {
                onboard::show(app.handle());
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Ultron failed to start");
}
