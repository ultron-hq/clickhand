//! The command line around the server: guided setup, health checks, connecting agents,
//! the kill switch and the log. Run with no arguments in a terminal, `ultron-hands` sets
//! itself up; started by an agent (stdin is a pipe), it serves MCP.
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::{desktop, input, log, screen};

pub const NAME: &str = "ultron-hands";

// ── output ──

fn color() -> bool {
    std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

fn paint(code: &str, s: &str) -> String {
    if color() {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

fn good(s: &str) {
    println!("  {} {s}", paint("32", "✓"));
}

fn warn(s: &str) {
    println!("  {} {s}", paint("33", "!"));
}

fn bad(s: &str) {
    println!("  {} {s}", paint("31", "✗"));
}

fn hint(s: &str) {
    println!("    {}", paint("2", s));
}

fn heading(s: &str) {
    println!("\n{}", paint("1", s));
}

/// Ask a yes/no question. Without a terminal, or with `--yes`, the default is the answer.
fn ask(question: &str, default_yes: bool, yes: bool) -> bool {
    let choices = if default_yes { "[Y/n]" } else { "[y/N]" };
    if yes || !std::io::stdin().is_terminal() {
        println!("  {question} {choices} {}", if default_yes { "yes" } else { "no" });
        return default_yes;
    }
    print!("  {question} {choices} ");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    if std::io::stdin().read_line(&mut line).is_err() {
        return default_yes;
    }
    match line.trim().to_lowercase().as_str() {
        "" => default_yes,
        a => a.starts_with('y'),
    }
}

// ── paths and programs ──

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_default()
}

/// This program's own path, which agents are pointed at (so PATH never matters).
pub fn exe() -> PathBuf {
    let me = std::env::current_exe().unwrap_or_else(|_| PathBuf::from(NAME));
    std::fs::canonicalize(&me).unwrap_or(me)
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// Find a program on PATH (with PATHEXT on Windows).
pub fn which(bin: &str) -> Option<PathBuf> {
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT".into())
            .split(';')
            .map(|e| e.to_lowercase())
            .chain([String::new()])
            .collect()
    } else {
        vec![String::new()]
    };
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|dir| exts.iter().map(|ext| dir.join(format!("{bin}{ext}"))).find(|p| p.is_file()))
}

/// Whether a configured command is this very program.
fn is_ours(cmd: &str) -> bool {
    let me = exe();
    if same_file(Path::new(cmd), &me) {
        return true;
    }
    !cmd.contains(['/', '\\']) && which(cmd).is_some_and(|p| same_file(&p, &me))
}

/// Run an agent's CLI (through `cmd /C` for .cmd shims on Windows); its stdout.
fn cli(bin: &Path, args: &[&str], check: bool) -> Result<String, String> {
    let p = bin.to_string_lossy().to_string();
    let shim = cfg!(windows) && bin.extension().is_some_and(|e| e == "cmd" || e == "bat");
    let mut cmd: Vec<&str> = if shim { vec!["cmd", "/C", &p] } else { vec![&p] };
    cmd.extend_from_slice(args);
    desktop::run(&cmd, 60.0, check).map_err(|e| e.0)
}

// ── agents ──

#[derive(Clone, Copy, PartialEq)]
enum Style {
    /// `{"mcpServers": {name: {"command", "args"}}}` (Cursor, Windsurf, Claude Desktop)
    Plain,
    /// VS Code's `{"servers": {name: {"type": "stdio", ...}}}`
    VsCode,
    /// Gemini CLI, which also takes `"trust": true` to skip confirmations
    Gemini,
}

enum Kind {
    ClaudeCode,
    Codex,
    Json { file: PathBuf, style: Style },
}

struct Agent {
    id: &'static str,
    name: &'static str,
    kind: Kind,
}

#[derive(Debug, PartialEq)]
enum Status {
    /// The app isn't installed.
    Missing,
    /// Installed, Hands not connected.
    Off,
    /// Connected to this program.
    Ours,
    /// Something else is registered under the same name.
    Other(String),
}

fn codex_config() -> PathBuf {
    std::env::var_os("CODEX_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".codex"))
        .join("config.toml")
}

fn claude_settings() -> PathBuf {
    std::env::var_os("CLAUDE_CONFIG_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".claude"))
        .join("settings.json")
}

fn agents() -> Vec<Agent> {
    let config = dirs::config_dir().unwrap_or_else(|| home().join(".config"));
    let json = |file: PathBuf, style| Kind::Json { file, style };
    vec![
        Agent {
            id: "claude-code",
            name: "Claude Code",
            kind: Kind::ClaudeCode,
        },
        Agent {
            id: "codex",
            name: "Codex",
            kind: Kind::Codex,
        },
        Agent {
            id: "cursor",
            name: "Cursor",
            kind: json(home().join(".cursor/mcp.json"), Style::Plain),
        },
        Agent {
            id: "vscode",
            name: "VS Code",
            kind: json(config.join("Code/User/mcp.json"), Style::VsCode),
        },
        Agent {
            id: "windsurf",
            name: "Windsurf",
            kind: json(home().join(".codeium/windsurf/mcp_config.json"), Style::Plain),
        },
        Agent {
            id: "claude-desktop",
            name: "Claude Desktop",
            kind: json(config.join("Claude/claude_desktop_config.json"), Style::Plain),
        },
        Agent {
            id: "gemini",
            name: "Gemini CLI",
            kind: json(home().join(".gemini/settings.json"), Style::Gemini),
        },
    ]
}

fn find_agent(id: &str) -> Option<Agent> {
    let id = match id {
        "claude" => "claude-code",
        "code" => "vscode",
        "gemini-cli" => "gemini",
        other => other,
    };
    agents().into_iter().find(|a| a.id == id)
}

fn json_key(style: Style) -> &'static str {
    if style == Style::VsCode {
        "servers"
    } else {
        "mcpServers"
    }
}

fn read_json(file: &Path) -> Result<Map<String, Value>, String> {
    match std::fs::read_to_string(file) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Map::new()),
        Err(e) => Err(format!("can't read {}: {e}", file.display())),
        Ok(t) if t.trim().is_empty() => Ok(Map::new()),
        Ok(t) => match serde_json::from_str::<Value>(&t) {
            Ok(Value::Object(m)) => Ok(m),
            _ => Err(format!("{} isn't plain JSON (comments?), so it was left alone", file.display())),
        },
    }
}

/// Write JSON through a temp file, keeping a one-time `.bak` of the user's original.
fn write_json(file: &Path, value: &Map<String, Value>) -> Result<(), String> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let bak = file.with_extension("json.bak");
    if file.exists() && !bak.exists() {
        let _ = std::fs::copy(file, &bak);
    }
    let text = serde_json::to_string_pretty(&Value::Object(value.clone())).map_err(|e| e.to_string())? + "\n";
    let tmp = file.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, file).map_err(|e| e.to_string())
}

/// The `[mcp_servers.ultron-hands]` section of Codex's config, as (start, end) line indexes.
fn codex_section(lines: &[&str]) -> Option<(usize, usize)> {
    let header = format!("[mcp_servers.{NAME}]");
    let alt = format!("[mcp_servers.\"{NAME}\"]");
    let start = lines.iter().position(|l| l.trim() == header || l.trim() == alt)?;
    let end = lines[start + 1..]
        .iter()
        .position(|l| l.trim_start().starts_with('['))
        .map_or(lines.len(), |i| start + 1 + i);
    Some((start, end))
}

fn toml_string(line: &str, key: &str) -> Option<String> {
    let (k, v) = line.split_once('=')?;
    (k.trim() == key).then(|| v.trim().trim_matches(['"', '\'']).to_string())
}

impl Agent {
    fn status(&self) -> Status {
        match &self.kind {
            Kind::ClaudeCode => {
                let Some(bin) = which("claude") else { return Status::Missing };
                let out = cli(&bin, &["mcp", "get", NAME], false).unwrap_or_default();
                match out.lines().find_map(|l| l.trim().strip_prefix("Command:")) {
                    Some(cmd) if is_ours(cmd.trim()) => Status::Ours,
                    Some(cmd) => Status::Other(cmd.trim().to_string()),
                    None => Status::Off,
                }
            }
            Kind::Codex => {
                if which("codex").is_none() {
                    return Status::Missing;
                }
                let text = std::fs::read_to_string(codex_config()).unwrap_or_default();
                let lines: Vec<&str> = text.lines().collect();
                let Some((s, e)) = codex_section(&lines) else { return Status::Off };
                match lines[s + 1..e].iter().find_map(|l| toml_string(l, "command")) {
                    Some(cmd) if is_ours(&cmd) => Status::Ours,
                    Some(cmd) => Status::Other(cmd),
                    None => Status::Off,
                }
            }
            Kind::Json { file, style } => {
                if !file.parent().is_some_and(Path::exists) {
                    return Status::Missing;
                }
                let Ok(cfg) = read_json(file) else {
                    return Status::Other("unreadable config".into());
                };
                match cfg.get(json_key(*style)).and_then(|s| s.get(NAME)) {
                    None => Status::Off,
                    Some(entry) => match entry["command"].as_str() {
                        Some(cmd) if is_ours(cmd) => Status::Ours,
                        Some(cmd) => Status::Other(cmd.to_string()),
                        None => Status::Other("a non-command entry".into()),
                    },
                }
            }
        }
    }

    /// Point the agent at this program; `approve` lets it act without asking each time.
    fn connect(&self, approve: bool) -> Result<String, String> {
        let me = exe().to_string_lossy().to_string();
        match &self.kind {
            Kind::ClaudeCode => {
                let bin = which("claude").ok_or("`claude` is not on PATH")?;
                let _ = cli(&bin, &["mcp", "remove", "-s", "user", NAME], false);
                cli(&bin, &["mcp", "add", "-s", "user", NAME, "--", &me], true)?;
                if approve {
                    allow_in_claude(true)?;
                }
                Ok("added for all your projects".into())
            }
            Kind::Codex => {
                let bin = which("codex").ok_or("`codex` is not on PATH")?;
                // Codex refuses to write its config into a CODEX_HOME that doesn't exist yet.
                if let Some(dir) = codex_config().parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                let _ = cli(&bin, &["mcp", "remove", NAME], false);
                cli(&bin, &["mcp", "add", NAME, "--", &me], true)?;
                if approve {
                    codex_approve(&codex_config()).map_err(|e| format!("added, but couldn't set auto-approve: {e}"))?;
                }
                Ok("added".into())
            }
            Kind::Json { file, style } => {
                let mut cfg = read_json(file)?;
                let mut entry = json!({"command": me, "args": []});
                if *style == Style::VsCode {
                    entry["type"] = json!("stdio");
                }
                if *style == Style::Gemini && approve {
                    entry["trust"] = json!(true);
                }
                let servers = cfg.entry(json_key(*style)).or_insert_with(|| json!({}));
                let Some(servers) = servers.as_object_mut() else {
                    return Err(format!("{} has an unexpected shape, left alone", file.display()));
                };
                servers.insert(NAME.into(), entry);
                write_json(file, &cfg)?;
                Ok(format!("added to {}", file.display()))
            }
        }
    }

    fn disconnect(&self) -> Result<(), String> {
        match &self.kind {
            Kind::ClaudeCode => {
                let bin = which("claude").ok_or("`claude` is not on PATH")?;
                cli(&bin, &["mcp", "remove", "-s", "user", NAME], true)?;
                allow_in_claude(false)
            }
            Kind::Codex => {
                let bin = which("codex").ok_or("`codex` is not on PATH")?;
                cli(&bin, &["mcp", "remove", NAME], true).map(|_| ())
            }
            Kind::Json { file, style } => {
                let mut cfg = read_json(file)?;
                if let Some(s) = cfg.get_mut(json_key(*style)).and_then(Value::as_object_mut) {
                    s.remove(NAME);
                }
                write_json(file, &cfg)
            }
        }
    }
}

/// Add (or remove) `mcp__ultron-hands` in Claude Code's allowed tools.
fn allow_in_claude(on: bool) -> Result<(), String> {
    let file = claude_settings();
    let rule = format!("mcp__{NAME}");
    let mut cfg = read_json(&file)?;
    let perms = cfg.entry("permissions").or_insert_with(|| json!({}));
    let Some(perms) = perms.as_object_mut() else {
        return Err("unexpected settings.json shape".into());
    };
    let allow = perms.entry("allow").or_insert_with(|| json!([]));
    let Some(allow) = allow.as_array_mut() else {
        return Err("unexpected settings.json shape".into());
    };
    let has = allow.iter().any(|v| v == &json!(rule));
    if on && !has {
        allow.push(json!(rule));
    } else if !on && has {
        allow.retain(|v| v != &json!(rule));
    } else {
        return Ok(());
    }
    write_json(&file, &cfg)
}

/// Set `default_tools_approval_mode = "approve"` in Codex's section for Hands.
fn codex_approve(file: &Path) -> std::io::Result<()> {
    let text = std::fs::read_to_string(file)?;
    let mut lines: Vec<&str> = text.lines().collect();
    let Some((s, e)) = codex_section(&lines) else {
        return Err(std::io::Error::other("no ultron-hands section"));
    };
    let setting = "default_tools_approval_mode = \"approve\"";
    match lines[s + 1..e]
        .iter()
        .position(|l| toml_string(l, "default_tools_approval_mode").is_some())
    {
        Some(i) => lines[s + 1 + i] = setting,
        None => lines.insert(s + 1, setting),
    }
    std::fs::write(file, lines.join("\n") + "\n")
}

// ── skills ──

const SKILLS: &[(&str, &str)] = &[
    ("ultron-hands", include_str!("../skills/ultron-hands/SKILL.md")),
    ("ultron-teach", include_str!("../skills/ultron-teach/SKILL.md")),
    ("ultron-post", include_str!("../skills/ultron-post/SKILL.md")),
];
const MANAGED: &str = "Installed by ultron-hands setup.";

/// Agent skill folders on this computer (Claude Code, Codex), for agents that are installed.
fn skill_dirs() -> Vec<(&'static str, PathBuf)> {
    let mut out = Vec::new();
    if which("claude").is_some() {
        out.push(("Claude Code", claude_settings().with_file_name("skills")));
    }
    if which("codex").is_some() {
        out.push(("Codex", codex_config().with_file_name("skills")));
    }
    out
}

/// Write the skills into `dir`, leaving any copy the person edited (marker removed) alone.
fn install_skills(dir: &Path) -> Result<usize, String> {
    let mut n = 0;
    for (name, body) in SKILLS {
        let file = dir.join(name).join("SKILL.md");
        if let Ok(old) = std::fs::read_to_string(&file) {
            if !old.contains(MANAGED) || old == *body {
                continue;
            }
        }
        std::fs::create_dir_all(file.parent().unwrap_or(dir)).map_err(|e| e.to_string())?;
        std::fs::write(&file, body).map_err(|e| format!("{}: {e}", file.display()))?;
        n += 1;
    }
    Ok(n)
}

pub fn skills() -> i32 {
    let dirs = skill_dirs();
    if dirs.is_empty() {
        warn("Neither Claude Code nor Codex is installed, so there's nowhere to add skills.");
        return 1;
    }
    let mut code = 0;
    for (agent, dir) in dirs {
        match install_skills(&dir) {
            Ok(_) => good(&format!(
                "{agent}: skills ultron-hands, ultron-teach and ultron-post in {}",
                dir.display()
            )),
            Err(e) => {
                bad(&format!("{agent}: {e}"));
                code = 1;
            }
        }
    }
    code
}

// ── doctor ──

#[cfg(target_os = "macos")]
mod mac {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGRequestScreenCaptureAccess() -> bool;
    }
    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }
    pub fn screen_ok() -> bool {
        unsafe { CGPreflightScreenCaptureAccess() }
    }
    pub fn ask_screen() {
        unsafe {
            CGRequestScreenCaptureAccess();
        }
    }
    pub fn input_ok() -> bool {
        unsafe { AXIsProcessTrusted() }
    }
    pub fn open_pane(pane: &str) {
        let url = format!("x-apple.systempreferences:com.apple.preference.security?{pane}");
        let _ = std::process::Command::new("open").arg(url).status();
    }
}

#[cfg(target_os = "linux")]
fn install_hint(pkg: &str) -> String {
    let cmd = [
        ("apt-get", "sudo apt install"),
        ("dnf", "sudo dnf install"),
        ("pacman", "sudo pacman -S"),
        ("zypper", "sudo zypper install"),
    ]
    .iter()
    .find(|(bin, _)| which(bin).is_some())
    .map_or("install", |(_, c)| c);
    format!("{cmd} {pkg}")
}

/// Check that Hands can see and act on this computer. Returns how many things must be fixed.
pub fn doctor() -> usize {
    let mut problems = 0;
    #[cfg(target_os = "linux")]
    if std::env::var("XDG_SESSION_TYPE").as_deref() == Ok("wayland") {
        warn("This is a Wayland session. Hands supports X11 for now.");
        hint("Log out, click the gear icon at the login screen and pick an \"Xorg\" session.");
    }
    match screen::screen_size() {
        Ok((w, h)) => good(&format!("Found your screen ({w}×{h})")),
        Err(e) => {
            problems += 1;
            bad(&format!("Can't find a screen: {}", e.0));
        }
    }
    #[cfg(target_os = "macos")]
    if !mac::screen_ok() {
        problems += 1;
        bad("Screen Recording permission is off, so Hands can't see the screen.");
        hint("Allow it for this terminal (or the agent app) in System Settings → Privacy & Security → Screen Recording, then reopen it.");
        mac::ask_screen();
        mac::open_pane("Privacy_ScreenCapture");
    }
    match screen::shot(320, None) {
        Ok(_) => good("Can take screenshots"),
        Err(e) => {
            problems += 1;
            bad(&format!("Can't take a screenshot: {}", e.0));
        }
    }
    #[cfg(target_os = "macos")]
    if !mac::input_ok() {
        problems += 1;
        bad("Accessibility permission is off, so Hands can't click or type.");
        hint("Allow it for this terminal (or the agent app) in System Settings → Privacy & Security → Accessibility.");
        mac::open_pane("Privacy_Accessibility");
    }
    match input::location() {
        Ok(_) => good("Can move the mouse and type"),
        Err(e) => {
            problems += 1;
            bad(&format!("Can't control the mouse: {}", e.0));
        }
    }
    #[cfg(target_os = "linux")]
    if which("xdotool").is_none() {
        warn("xdotool is missing, so listing and focusing windows won't work (everything else will).");
        hint(&format!("Fix: {}", install_hint("xdotool")));
    }
    if log::stop_path().exists() {
        warn("The kill switch is on, so Hands won't act.");
        hint("Turn it off with: ultron-hands resume");
    }
    if log::readonly() {
        warn("ULTRON_HANDS_READONLY=1 is set: Hands can look but not act.");
    }
    problems
}

// ── commands ──

const TRY: &str = "Use ultron-hands to open getultron.vercel.app and press the big red button";

fn label(status: &Status) -> String {
    match status {
        Status::Missing => "not installed".into(),
        Status::Off => "not connected".into(),
        Status::Ours => "connected".into(),
        Status::Other(cmd) => format!("uses a different {NAME} ({cmd})"),
    }
}

/// The guided first run: check the computer, connect every agent found, show how to use it.
pub fn setup(yes: bool) -> i32 {
    println!(
        "{}  {}",
        paint("1;31", "Ultron Hands"),
        paint("2", concat!("v", env!("CARGO_PKG_VERSION")))
    );
    println!("Lets your AI agent see this screen, click and type, like a person at the computer.");

    heading("1. Checking this computer");
    let problems = doctor();

    heading("2. Connecting your agents");
    let found: Vec<(Agent, Status)> = agents()
        .into_iter()
        .map(|a| {
            let s = a.status();
            (a, s)
        })
        .filter(|(_, s)| *s != Status::Missing)
        .collect();
    if found.is_empty() {
        warn("No AI agents found on this computer.");
        hint("Install Claude Code (claude.com/claude-code) or Codex, then run: ultron-hands setup");
        hint(&format!(
            "Any other MCP app: add a stdio server with the command {}",
            exe().display()
        ));
    }
    let todo: Vec<&(Agent, Status)> = found.iter().filter(|(_, s)| *s != Status::Ours).collect();
    let approve = todo
        .iter()
        .any(|(a, _)| matches!(a.kind, Kind::ClaudeCode | Kind::Codex | Kind::Json { style: Style::Gemini, .. }))
        && ask(
            "Let agents use Hands without asking you before each action? (the kill switch still stops them)",
            true,
            yes,
        );
    let mut connected = 0;
    for (agent, status) in &found {
        let replace = match status {
            Status::Ours => {
                good(&format!("{}: already connected", agent.name));
                connected += 1;
                continue;
            }
            Status::Other(cmd) => ask(
                &format!(
                    "{} already has a server called {NAME} ({cmd}). Replace it with this one?",
                    agent.name
                ),
                false,
                yes,
            ),
            _ => true,
        };
        if !replace {
            warn(&format!("{}: left as it was", agent.name));
            continue;
        }
        match agent.connect(approve) {
            Ok(how) => {
                connected += 1;
                good(&format!("{}: {how}", agent.name));
            }
            Err(e) => bad(&format!("{}: {e}", agent.name)),
        }
    }

    for (agent, dir) in skill_dirs() {
        match install_skills(&dir) {
            Ok(_) => good(&format!("{agent}: skills /ultron-hands, /ultron-teach and /ultron-post")),
            Err(e) => warn(&format!("{agent}: couldn't add the skills ({e})")),
        }
    }

    heading("3. Try it");
    if connected > 0 {
        println!("  Open your agent (restart it if it was running) and ask:");
        println!("    {}", paint("36", &format!("\"{TRY}\"")));
        println!("  Or learn an app on your own screen:");
        println!("    {}", paint("36", "\"/ultron-teach show me how to export a PNG in Figma\""));
    }
    println!(
        "  Stop it at any moment:  {}    (undo: ultron-hands resume)",
        paint("1", "ultron-hands stop")
    );
    println!("  See what it did:        ultron-hands log");
    println!("  Check again:            ultron-hands doctor");
    if problems > 0 {
        println!("\n{}", paint("33", "Fix the items marked ✗ above, then run: ultron-hands doctor"));
        return 1;
    }
    0
}

pub fn status_list() {
    for a in agents() {
        let s = a.status();
        let line = format!("{:<15} {:<15} {}", a.id, a.name, label(&s));
        match s {
            Status::Ours => good(&line),
            Status::Missing => println!("    {}", paint("2", &line)),
            _ => warn(&line),
        }
    }
}

pub fn connect(id: Option<&str>, on: bool, yes: bool) -> i32 {
    let Some(agent) = id.and_then(find_agent) else {
        eprintln!("Which agent? One of:");
        status_list();
        return 2;
    };
    if !on {
        return match agent.disconnect() {
            Ok(()) => {
                good(&format!("{}: disconnected", agent.name));
                0
            }
            Err(e) => {
                bad(&format!("{}: {e}", agent.name));
                1
            }
        };
    }
    let status = agent.status();
    if status == Status::Missing {
        bad(&format!("{} doesn't seem to be installed.", agent.name));
        return 1;
    }
    if let Status::Other(cmd) = &status {
        if !ask(&format!("{} already has {NAME} ({cmd}). Replace it?", agent.name), false, yes) {
            return 1;
        }
    }
    let approve = ask("Let it use Hands without asking before each action?", true, yes);
    match agent.connect(approve) {
        Ok(how) => {
            good(&format!("{}: {how}", agent.name));
            0
        }
        Err(e) => {
            bad(&format!("{}: {e}", agent.name));
            1
        }
    }
}

pub fn stop(on: bool) -> i32 {
    let path = log::stop_path();
    if on {
        if let Err(e) = std::fs::write(&path, "stopped from the command line\n") {
            bad(&format!("Couldn't turn on the kill switch: {e}"));
            return 1;
        }
        good("Stopped. Hands will refuse every click and keystroke until you run: ultron-hands resume");
    } else {
        let _ = std::fs::remove_file(&path);
        good("Resumed. Agents can use Hands again.");
    }
    0
}

pub fn show_log(n: usize) -> i32 {
    let recs = log::read(n);
    if recs.is_empty() {
        println!("Nothing yet. Every action Hands takes is listed here.");
        return 0;
    }
    let now = log::now();
    for r in recs {
        let ago = (now - r["ts"].as_f64().unwrap_or(now)).max(0.0) as u64;
        let when = match ago {
            0..60 => format!("{ago}s ago"),
            60..3600 => format!("{}m ago", ago / 60),
            3600..86400 => format!("{}h ago", ago / 3600),
            _ => format!("{}d ago", ago / 86400),
        };
        let tool = r["tool"].as_str().unwrap_or("?");
        let args = r.get("args").map(|a| a.to_string()).unwrap_or_default();
        let args: String = args.chars().take(70).collect();
        let line = format!("{when:>8}  {tool:<13} {args}");
        if r["ok"] == json!(true) || tool == "feedback" {
            println!("  {line}");
        } else {
            println!("  {}  {}", paint("31", &line), r["error"].as_str().unwrap_or(""));
        }
    }
    0
}

pub const HELP: &str = "\
Ultron Hands: let Claude Code, Codex or any MCP agent see your screen, click and type.

Usage: ultron-hands [command]

  (no command)        in a terminal: guided setup; started by an agent: the MCP server
  setup [--yes]       check this computer and connect every agent found
  doctor              check the screen, input and permissions
  agents              list the agents on this computer and whether they're connected
  connect <agent>     connect one agent: claude-code, codex, cursor, vscode,
                      windsurf, claude-desktop, gemini
  disconnect <agent>  remove Hands from an agent
  stop                kill switch: refuse every action until `resume`
  resume              turn the kill switch off
  log [n]             the last n actions (default 20)
  skills              (re)install the ultron-hands, ultron-teach and ultron-post skills
  serve               run the MCP server on stdin/stdout (what agents start)
  --version           print the version

Docs: https://github.com/ultron-hq/ultron";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skills_install_and_respect_edits() {
        let dir = std::env::temp_dir().join(format!("uh-skills-{}", std::process::id()));
        assert_eq!(install_skills(&dir).unwrap(), 3);
        assert_eq!(install_skills(&dir).unwrap(), 0, "unchanged copies are left alone");
        let teach = dir.join("ultron-teach/SKILL.md");
        assert!(std::fs::read_to_string(&teach).unwrap().starts_with("---\nname: ultron-teach\n"));
        std::fs::write(&teach, "my own version").unwrap();
        install_skills(&dir).unwrap();
        assert_eq!(std::fs::read_to_string(&teach).unwrap(), "my own version");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn codex_section_and_approval() {
        let dir = std::env::temp_dir().join(format!("uh-codex-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("config.toml");
        let text =
            "model = \"x\"\n\n[mcp_servers.ultron-hands]\ncommand = \"/bin/uh\"\nargs = []\n\n[mcp_servers.other]\ncommand = \"o\"\n";
        std::fs::write(&file, text).unwrap();
        codex_approve(&file).unwrap();
        codex_approve(&file).unwrap(); // idempotent
        let out = std::fs::read_to_string(&file).unwrap();
        assert_eq!(out.matches("default_tools_approval_mode").count(), 1);
        let lines: Vec<&str> = out.lines().collect();
        let (s, e) = codex_section(&lines).unwrap();
        assert!(lines[s + 1..e].contains(&"default_tools_approval_mode = \"approve\""));
        assert!(lines[e].starts_with("[mcp_servers.other]"));
        assert_eq!(lines[s + 1..e].iter().find_map(|l| toml_string(l, "command")).unwrap(), "/bin/uh");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn json_merge_keeps_other_servers_and_backs_up() {
        let dir = std::env::temp_dir().join(format!("uh-json-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("mcp.json");
        std::fs::write(&file, r#"{"mcpServers": {"other": {"command": "o"}}, "theme": "dark"}"#).unwrap();
        let agent = Agent {
            id: "t",
            name: "T",
            kind: Kind::Json {
                file: file.clone(),
                style: Style::Plain,
            },
        };
        assert_eq!(agent.status(), Status::Off);
        agent.connect(true).unwrap();
        assert_eq!(agent.status(), Status::Ours);
        let cfg = read_json(&file).unwrap();
        assert_eq!(cfg["mcpServers"]["other"]["command"], "o");
        assert_eq!(cfg["theme"], "dark");
        assert!(file.with_extension("json.bak").exists());
        agent.disconnect().unwrap();
        assert_eq!(agent.status(), Status::Off);
        std::fs::write(&file, "{ // comment\n}").unwrap();
        assert!(agent.connect(true).is_err(), "JSONC is left alone");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vscode_and_gemini_shapes() {
        let dir = std::env::temp_dir().join(format!("uh-shape-{}", std::process::id()));
        for (style, key) in [(Style::VsCode, "servers"), (Style::Gemini, "mcpServers")] {
            let file = dir.join(format!("{key}.json"));
            let agent = Agent {
                id: "t",
                name: "T",
                kind: Kind::Json { file: file.clone(), style },
            };
            agent.connect(true).unwrap();
            let e = &read_json(&file).unwrap()[key][NAME];
            assert_eq!(e["type"] == json!("stdio"), style == Style::VsCode);
            assert_eq!(e["trust"] == json!(true), style == Style::Gemini);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
