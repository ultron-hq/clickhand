//! Front door (rust-anything whole-app step 4), ported from ultron/__main__.py.
//!
//! Same flags and exit codes as the original. Messages to a running Ultron are
//! handled here in Rust; starting Ultron itself still falls back to the Python
//! original ($ULTRON_ORIGINAL, default ~/.local/bin/ultron) until the Tauri app
//! (rust/MAP.md wave 3) replaces it.
use std::process::{Command, ExitCode};
use std::time::Duration;

use ultron_core::ipc::{self, Message};

const HELP: &str = "ultron              start Ultron (island + agent)
ultron --prompt     open the prompt bar (starts Ultron first if needed); bind to Ctrl+Space
ultron --cursor     open beside the mouse cursor; bind to Ctrl+Shift+Space
ultron --voice      toggle voice input; bind to Ctrl+Alt+Space
ultron --quit       stop the running Ultron
ultron --keepalive  start and restart after a crash (used by the login autostart)
ultron --say TEXT   show TEXT in the island
ultron --version    print the version
";

fn original() -> String {
    std::env::var("ULTRON_ORIGINAL").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_default();
        format!("{home}/.local/bin/ultron")
    })
}

/// Hand the whole invocation to the original and pass its exit code through.
fn fallback(args: &[String]) -> ExitCode {
    if let Ok(log) = std::env::var("RUST_ANYTHING_FALLBACK_LOG") {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log) {
            let _ = writeln!(f, "{}", args.join(" "));
        }
    }
    match Command::new(original()).args(args).status() {
        Ok(s) => ExitCode::from(s.code().unwrap_or(1) as u8),
        Err(e) => {
            eprintln!("Can't start Ultron ({}): {e}", original());
            ExitCode::from(1)
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |f: &str| args.iter().any(|a| a == f);

    if has("-V") || has("--version") {
        println!("ultron {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    if has("-h") || has("--help") {
        println!("{HELP}");
        return ExitCode::SUCCESS;
    }
    if has("--quit") {
        return if ipc::send(&Message::Quit) {
            ExitCode::SUCCESS
        } else {
            ExitCode::from(1)
        };
    }
    if let Some(i) = args.iter().position(|a| a == "--say") {
        let text = args[i + 1..].join(" ").replace('\n', " ").trim().to_string();
        return if !text.is_empty() && ipc::send(&Message::Say(text)) {
            ExitCode::SUCCESS
        } else {
            ExitCode::from(1)
        };
    }
    if has("--keepalive") {
        return fallback(&args); // the supervisor loop lives in the original's launcher for now
    }

    let voice = has("--voice");
    let at_cursor = has("--cursor");
    let prompt = has("--prompt") || at_cursor || voice;
    let msg = if voice {
        Message::Voice
    } else if at_cursor {
        Message::Cursor
    } else if prompt {
        Message::Prompt
    } else {
        Message::Ping
    };
    if ipc::send(&msg) {
        if !prompt {
            println!("Ultron is already running.");
        }
        return ExitCode::SUCCESS;
    }

    // Nothing answered. If another Ultron holds the lock it is starting or stopping.
    match ipc::lock(Duration::ZERO) {
        Ok(Some(held)) => {
            drop(held); // free: let the original take it
            fallback(&args)
        }
        Ok(None) => {
            if ipc::deliver(&msg, 12.0) {
                return ExitCode::SUCCESS;
            }
            match ipc::lock(Duration::from_secs(10)) {
                Ok(Some(held)) => {
                    drop(held);
                    fallback(&args)
                }
                _ => {
                    eprintln!("Another Ultron is stuck; see ~/.cache/ultron/ultron.log");
                    ExitCode::from(1)
                }
            }
        }
        Err(e) => {
            eprintln!("Can't open the Ultron lock: {e}");
            ExitCode::from(1)
        }
    }
}
