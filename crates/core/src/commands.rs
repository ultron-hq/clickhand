//! Explicit shortcuts and bounded, read-only context references for the prompt,
//! ported from ultron/commands.py: slash commands, @ tags, "build me a …" requests and
//! opening installed abilities.
//!
//! Some commands are the founder's own integrations (THE SYSTEM, deen, Simora paper
//! trading). They are `personal` (ADR-014): off in the public build, on when asked for.
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use serde_json::{json, Map, Value};

/// (name, description, usage, personal)
pub const COMMANDS: &[(&str, &str, &str, bool)] = &[
    ("/build", "Build anything as a new /command", "<what you want>", false),
    ("/abilities", "Open your installed abilities", "", false),
    ("/ability", "Open an installed ability by name", "<name>", false),
    ("/hacker", "Audit an authorized local project", "--authorized <path> | tools", false),
    ("/agents", "Collaborate with parallel specialists", "[--count 2–4] <task>", false),
    ("/deploy", "Deploy a web app to Vercel", "<ability name | folder> | list", false),
    (
        "/workspace",
        "Open Ultron Workspace: chat, research, docs, email, notes, calendar",
        "[notes|calendar|email|tasks|memory|gallery|cookbook|library|status|restart|stop]",
        false,
    ),
    (
        "/model",
        "Choose the desktop or routing model",
        "[desktop|router] [haiku|sonnet|opus]",
        false,
    ),
    ("/effort", "Set desktop reasoning effort", "[low|medium|high]", false),
    ("/speed", "Apply a speed and reasoning preset", "[fast|balanced|thorough]", false),
    ("/settings", "Show active models and effort", "", false),
    ("/voice", "Start or end a voice conversation", "", false),
    ("/commands", "Show commands and context tags", "", false),
    ("/today", "Today's quests from THE SYSTEM", "", true),
    ("/quest", "Add a quest for today in THE SYSTEM", "<objective>", true),
    ("/plan", "Plan a quest for tomorrow", "<objective>", true),
    ("/did", "Mark a quest in THE SYSTEM done", "<quest name>", true),
    ("/log", "Log sleep, study, quran, money, weight or water", "<kind> <values>", true),
    ("/dua", "A supplication / adhkar (UmmahAPI)", "[morning|evening]", true),
    ("/ayah", "A Qur\u{2019}an verse (UmmahAPI)", "", true),
    ("/task", "Add a task", "<title>", false),
    ("/tasks", "List your active tasks", "", false),
    ("/focus", "Focus on a task for 25 minutes", "<task ID>", false),
    ("/done", "Complete a task", "<task ID>", false),
    ("/timer", "Start a focus timer", "<minutes> [label]", false),
    ("/research", "Research with source links", "<question>", false),
    ("/teach", "Learn one step at a time", "<goal>", false),
    ("/analyze", "Analyze the screen or attached context", "<question>", false),
    ("/do", "Carry out a desktop task", "<goal>", false),
    ("/paper", "Start a bounded Simora paper session", "[goal]", true),
    ("/report", "Open your time and task report", "", false),
    ("/runs", "Summarize recent runs", "", false),
    ("/new", "Clear conversation context", "", false),
    ("/stop", "Stop the current task", "", false),
];
const SYSTEM_COMMANDS: [&str; 5] = ["/today", "/quest", "/plan", "/did", "/log"];
const DEEN_COMMANDS: [&str; 2] = ["/dua", "/ayah"];
pub const TAGS: [(&str, &str); 4] = [
    ("@screen", "Use the current screen"),
    ("@tasks", "Attach up to 20 active tasks"),
    ("@activity", "Attach the last five run summaries"),
    ("@task:", "Attach one task by ID"),
];

// A build request names something buildable within a few words ("make me a habit tracker"), or says "build me".
// Ordinary chat ("make me laugh", "create a task to run the command") must not start a build.
static BUILD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:please\s+|can you\s+|could you\s+)?(?:(?:build|create|make)\s+(?:me\s+)?(?:a|an|my|some)\s+(?:\S+\s+){0,3}?(?:apps?|abilit(?:y|ies)|tools?|dashboards?|commands?|trackers?|games?|calculators?|widgets?|converters?|generators?)\b|build\s+me\b)").unwrap()
});
static TAG: LazyLock<fancy_regex::Regex> =
    LazyLock::new(|| fancy_regex::Regex::new(r"(?i)(?<!\S)@(screen|tasks|activity|task:\d+)\b(?![\w@./:-])").unwrap());
static BAD_TASK_TAG: LazyLock<fancy_regex::Regex> = LazyLock::new(|| fancy_regex::Regex::new(r"(?i)(?<!\S)@task:(?!\d+\b)").unwrap());
static NAMED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(?:open|show|use|run)\s+(?:my\s+)?(.+)$").unwrap());
static ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#?\d+$").unwrap());
static TIMER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d{1,3})(?:\s+(.+))?$").unwrap());

/// An installed generated ability (the registry itself lands in wave 6).
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct Generated {
    pub id: String,
    pub name: String,
    pub command: String,
    pub usage: String,
    pub description: String,
}

/// What `prepare` reads: the shared database (read-only), installed abilities, and
/// whether the personal integrations are on.
pub struct Context {
    pub db: PathBuf,
    pub generated: Vec<Generated>,
    pub personal: bool,
}

impl Context {
    fn commands(&self) -> impl Iterator<Item = &(&'static str, &'static str, &'static str, bool)> {
        COMMANDS.iter().filter(|c| self.personal || !c.3)
    }

    /// Read-only query; missing database or table → no rows, a busy database → an error.
    fn rows(&self, sql: &str, args: &[&dyn rusqlite::ToSql]) -> Result<Vec<Map<String, Value>>, String> {
        if !self.db.exists() {
            return Ok(vec![]);
        }
        let conn = Connection::open_with_flags(&self.db, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI).map_err(busy)?;
        let _ = conn.busy_timeout(std::time::Duration::from_millis(200));
        let mut st = match conn.prepare(sql) {
            Ok(st) => st,
            Err(e) if e.to_string().contains("no such table") => return Ok(vec![]),
            Err(e) => return Err(busy(e)),
        };
        let names: Vec<String> = st.column_names().iter().map(|s| s.to_string()).collect();
        let rows = st
            .query_map(args, |r| {
                let mut m = Map::new();
                for (i, n) in names.iter().enumerate() {
                    let v = match r.get_ref(i)? {
                        rusqlite::types::ValueRef::Null => Value::Null,
                        rusqlite::types::ValueRef::Integer(x) => json!(x),
                        rusqlite::types::ValueRef::Real(x) => json!(x),
                        rusqlite::types::ValueRef::Text(t) => json!(String::from_utf8_lossy(t)),
                        rusqlite::types::ValueRef::Blob(_) => Value::Null,
                    };
                    m.insert(n.clone(), v);
                }
                Ok(m)
            })
            .map_err(busy)?;
        rows.collect::<rusqlite::Result<_>>().map_err(busy)
    }

    pub fn active_tasks(&self) -> Result<Vec<Map<String, Value>>, String> {
        self.rows(
            "SELECT id,title,status,priority,due FROM tasks WHERE status!='done' ORDER BY id DESC LIMIT 20",
            &[],
        )
    }

    pub fn recent_runs(&self) -> Result<Vec<Map<String, Value>>, String> {
        self.rows(
            "SELECT id,goal,status,substr(summary,1,600) AS summary FROM agent_runs ORDER BY id DESC LIMIT 5",
            &[],
        )
    }

    fn installed(&self, name: &str) -> Result<Vec<Map<String, Value>>, String> {
        self.rows(
            "SELECT id FROM abilities WHERE active_version IS NOT NULL AND lower(name)=lower(?1)",
            &[&name],
        )
    }

    pub fn help(&self) -> String {
        let lines: Vec<String> = self.commands().map(|(n, d, u, _)| command_line(n, u, d)).collect();
        let yours = if self.generated.is_empty() {
            "\n\nBuild your own: /build <anything you want>".to_string()
        } else {
            format!(
                "\n\nYour abilities\n{}",
                self.generated
                    .iter()
                    .map(|a| command_line(&a.command, &a.usage, &a.description))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        };
        let tags: Vec<String> = TAGS.iter().map(|(n, d)| format!("{n} — {d}")).collect();
        format!(
            "Commands\n{}{yours}\n\nContext tags\n{}\n\nExample: /analyze @tasks What should I work on first?",
            lines.join("\n"),
            tags.join("\n")
        )
    }
}

fn busy(e: rusqlite::Error) -> String {
    let _ = e;
    "Local context is busy. Try again in a moment.".into()
}

pub fn command_line(name: &str, usage: &str, desc: &str) -> String {
    let head = [name, usage]
        .iter()
        .filter(|s| !s.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" ");
    format!("{head} — {desc}")
}

/// A request after the command layer: either a reply, a settings command, or a goal
/// (with an optional fixed skill, mode, references, literal title and timer).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Request {
    pub goal: String,
    pub reply: Option<String>,
    pub skill: Option<String>,
    pub mode: Option<String>,
    pub references: Vec<Value>,
    pub literal_title: Option<String>,
    pub timer: Option<(u32, Option<String>)>,
    pub settings_command: Option<String>,
}

fn reply(text: impl Into<String>) -> Request {
    Request {
        reply: Some(text.into()),
        ..Request::default()
    }
}
fn to(goal: impl Into<String>, skill: &str) -> Request {
    Request {
        goal: goal.into(),
        skill: Some(skill.into()),
        ..Request::default()
    }
}

pub fn prepare(text: &str, ctx: &Context) -> Result<Request, String> {
    let text = text.trim();
    let mut result = Request {
        goal: text.into(),
        ..Request::default()
    };
    if text.is_empty() {
        return Ok(reply("Type a request, /help, or @ to attach context."));
    }
    if !text.starts_with('/') && BUILD.is_match(text) {
        return Ok(to(text, "ability_builder"));
    }
    if let Some(named) = NAMED.captures(text) {
        let wanted = named[1].trim().to_lowercase();
        if let Some(a) = ctx.generated.iter().find(|a| a.name.to_lowercase() == wanted) {
            return Ok(to(format!("@generated:{}", a.id), "generated"));
        }
        let found = ctx.installed(named[1].trim())?;
        if found.len() == 1 {
            return Ok(to(
                format!("@ability:{}", found[0]["id"].as_str().unwrap_or_default()),
                "ability_builder",
            ));
        }
    }
    let mut command: Option<String> = None;
    let mut body = text.to_string();
    if text.starts_with('/') {
        let mut parts = text.splitn(2, char::is_whitespace);
        let cmd = parts.next().unwrap_or_default().to_lowercase();
        body = parts.next().unwrap_or_default().trim().to_string();
        let known = cmd == "/help" || ctx.commands().any(|c| c.0 == cmd);
        if !known {
            if let Some(a) = ctx.generated.iter().find(|a| a.command == cmd) {
                return Ok(to(format!("@generated:{} {body}", a.id).trim_end(), "generated"));
            }
            return Ok(reply(format!("Unknown command {cmd}. Type /help for the available commands.")));
        }
        let c = cmd.as_str();
        if ["/model", "/effort", "/speed", "/settings"].contains(&c) {
            return Ok(Request {
                settings_command: Some(text.into()),
                ..Request::default()
            });
        }
        if c == "/voice" {
            return Ok(reply("Use the microphone button or Ctrl+Alt+Space to start or end voice."));
        }
        if SYSTEM_COMMANDS.contains(&c) {
            return Ok(to(format!("{c} {body}").trim(), "system")); // arguments verbatim
        }
        if DEEN_COMMANDS.contains(&c) {
            return Ok(to(format!("{c} {body}").trim(), "deen"));
        }
        if c == "/ability" {
            let found = if body.is_empty() { vec![] } else { ctx.installed(&body)? };
            if found.len() == 1 {
                return Ok(to(
                    format!("@ability:{}", found[0]["id"].as_str().unwrap_or_default()),
                    "ability_builder",
                ));
            }
            return Ok(reply(
                "Choose an installed ability by its exact name, or use /abilities to open the library.",
            ));
        }
        if c == "/build" || c == "/abilities" {
            return Ok(to(if c == "/build" { body.as_str() } else { "" }, "ability_builder"));
        }
        if c == "/deploy" {
            return Ok(to(body, "deploy"));
        }
        if c == "/workspace" {
            return Ok(to(format!("workspace {body}").trim(), "workspace"));
        }
        if c == "/hacker" || c == "/agents" {
            if !body.is_empty() {
                return Ok(to(body, &c[1..]));
            }
            return Ok(reply(if c == "/hacker" {
                "Usage: /hacker --authorized <local-project-path> or /hacker tools"
            } else {
                "Usage: /agents [--count 2–4] <task>. Parallel text specialists followed by a reviewer."
            }));
        }
        if c == "/help" || c == "/commands" {
            return Ok(reply(ctx.help()));
        }
        if ["/task", "/focus", "/done", "/timer", "/new", "/stop", "/report", "/tasks", "/runs"].contains(&c) {
            // Literal task titles and labels must not be read as context tags.
            if c == "/task" {
                if body.is_empty() {
                    return Ok(reply("Usage: /task <title>"));
                }
                return Ok(Request {
                    literal_title: Some(body.clone()),
                    ..to(format!("add task {body}"), "tasks")
                });
            }
            if c == "/focus" || c == "/done" {
                if !ID.is_match(&body) {
                    return Ok(reply(format!("Usage: {c} <task ID>, for example {c} 8")));
                }
                let verb = if c == "/focus" { "focus" } else { "complete" };
                return Ok(to(format!("{verb} task {}", body.trim_start_matches('#')), "tasks"));
            }
            if c == "/timer" {
                let m = TIMER.captures(&body);
                let mins = m.as_ref().and_then(|m| m[1].parse::<u32>().ok()).filter(|n| (1..=240).contains(n));
                let (Some(m), Some(mins)) = (m, mins) else {
                    return Ok(reply("Usage: /timer <1–240 minutes> [label]"));
                };
                let label = m.get(2).map(|l| l.as_str().to_string());
                let goal = format!(
                    "timer {mins} min{}",
                    label.as_ref().map(|l| format!(" for {l}")).unwrap_or_default()
                );
                return Ok(Request {
                    timer: Some((mins * 60, label)),
                    ..to(goal, "timer")
                });
            }
            if !body.is_empty() {
                return Ok(reply(format!("{c} takes no arguments.")));
            }
            return Ok(match c {
                "/new" => Request {
                    goal: "new conversation".into(),
                    ..Request::default()
                },
                "/stop" => reply("Stopped. You have control."),
                "/report" => to("show my week report", "time_report"),
                "/tasks" => {
                    let items = ctx.active_tasks()?;
                    if items.is_empty() {
                        reply("No active tasks. Use /task <title> to add one.")
                    } else {
                        let lines: Vec<String> = items
                            .iter()
                            .map(|t| format!("#{} · {} [{}]", t["id"], s(&t["title"]), s(&t["status"])))
                            .collect();
                        reply(format!("Active tasks\n{}", lines.join("\n")))
                    }
                }
                _ => {
                    let items = ctx.recent_runs()?;
                    if items.is_empty() {
                        reply("No runs yet.")
                    } else {
                        let blocks: Vec<String> = items
                            .iter()
                            .map(|r| format!("#{} · {} · {}\n{}", r["id"], s(&r["status"]), s(&r["goal"]), s(&r["summary"])))
                            .collect();
                        reply(format!("Recent activity\n{}", blocks.join("\n\n")))
                    }
                }
            });
        }
        command = Some(cmd);
    }

    let mut seen: Vec<String> = Vec::new();
    for m in TAG.captures_iter(&body) {
        let Ok(m) = m else { continue };
        let tag = m.get(1).map(|g| g.as_str().to_lowercase()).unwrap_or_default();
        if seen.contains(&tag) {
            continue;
        }
        seen.push(tag.clone());
        if seen.len() > 5 {
            return Ok(reply("Attach at most five context tags per request."));
        }
        let value = match tag.as_str() {
            "screen" => json!("Use the fresh screenshot of the current screen."),
            "tasks" => json!(ctx.active_tasks()?),
            "activity" => json!(ctx.recent_runs()?),
            _ => {
                let id: i64 = tag[5..].parse().unwrap_or(-1);
                let found = ctx.rows("SELECT id,title,status,priority,due FROM tasks WHERE id=?1", &[&id])?;
                let Some(first) = found.into_iter().next() else {
                    return Ok(reply(format!(
                        "Task #{} was not found. Use /tasks to see available IDs.",
                        &tag[5..]
                    )));
                };
                Value::Object(first)
            }
        };
        result.references.push(json!({"tag": format!("@{tag}"), "data": value}));
    }
    let mut body = TAG.replace_all(&body, "").trim().to_string();
    if BAD_TASK_TAG.is_match(&body).unwrap_or(false) {
        return Ok(reply("Use @task:<ID>, for example @task:8."));
    }
    if let Some(cmd) = command.as_deref() {
        if cmd == "/paper" {
            let goal = if body.is_empty() {
                "Compare setups and run a bounded paper session, then review it."
            } else {
                body.as_str()
            };
            body = format!("Paper trade using Simora on trade.padre.gg. {goal}");
        } else if body.is_empty() {
            let (n, _, u, _) = COMMANDS.iter().find(|c| c.0 == cmd).unwrap();
            return Ok(reply(format!("Usage: {n} {u}")));
        } else {
            let prefix = match cmd {
                "/research" => "Research ",
                "/teach" => "Teach me ",
                "/analyze" => "Analyze ",
                _ => "",
            };
            body = format!("{prefix}{body}");
        }
        result.skill = Some(if cmd == "/do" { "desktop" } else { "workflow" }.into());
        result.mode = Some(
            match cmd {
                "/research" => "research",
                "/teach" => "teach",
                "/analyze" => "analyze",
                "/do" => "execute",
                _ => "paper",
            }
            .into(),
        );
    } else if !result.references.is_empty() {
        if body.is_empty() {
            return Ok(reply(
                "Add a question after the tag, for example @tasks What should I work on first?",
            ));
        }
        result.skill = Some("workflow".into());
        result.mode = Some("analyze".into());
    }
    result.goal = body;
    Ok(result)
}

fn s(v: &Value) -> String {
    v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())
}

/// Options offered while typing after a settings command, longest prefix first.
pub fn options() -> Vec<(&'static str, Vec<(&'static str, String)>)> {
    let models = ["haiku", "sonnet", "opus"];
    let model_desc = |m: &str| match m {
        "haiku" => "Fast, light tasks",
        "sonnet" => "Balanced desktop reasoning",
        _ => "Complex reasoning",
    };
    let mut model: Vec<(&str, String)> = models.iter().map(|m| (*m, model_desc(m).to_string())).collect();
    model.push(("router", "Choose the fallback routing model".into()));
    vec![
        ("/model", model),
        (
            "/model router",
            models.iter().map(|m| (*m, "Fallback routing model".to_string())).collect(),
        ),
        (
            "/model desktop",
            models.iter().map(|m| (*m, "Desktop planning model".to_string())).collect(),
        ),
        (
            "/effort",
            vec![
                ("low", "Faster, less deliberation".into()),
                ("medium", "Balanced reasoning".into()),
                ("high", "More reasoning, potentially slower".into()),
            ],
        ),
        (
            "/speed",
            vec![
                ("fast", "Desktop: haiku · effort low".into()),
                ("balanced", "Desktop: sonnet · effort medium".into()),
                ("thorough", "Desktop: opus · effort high".into()),
            ],
        ),
    ]
}

/// The span to replace (in characters) and the (name, description) choices.
pub type Suggestions = (Option<(usize, usize)>, Vec<(String, String)>);

/// Autocomplete for the prompt bar: the (start, end) span to replace, in characters,
/// and the (name, description) choices.
pub fn suggestions(text: &str, cursor: Option<usize>, ctx: &Context) -> Result<Suggestions, String> {
    let chars: Vec<char> = text.chars().collect();
    let cursor = cursor.unwrap_or(chars.len()).min(chars.len());
    let before: String = chars[..cursor].iter().collect();
    let mut opts = options();
    opts.sort_by_key(|(p, _)| std::cmp::Reverse(p.len()));
    for (prefix, choices) in opts {
        let re = Regex::new(&format!(r"(?i)^\s*{}\s+(\S*)$", regex::escape(prefix))).unwrap();
        if let Some(m) = re.captures(&before) {
            let tok = m.get(1).unwrap();
            let token = tok.as_str().to_lowercase();
            let start = before[..tok.start()].chars().count();
            return Ok((
                Some((start, cursor)),
                choices
                    .into_iter()
                    .filter(|(n, _)| n.starts_with(&token))
                    .map(|(n, d)| (n.to_string(), d))
                    .collect(),
            ));
        }
    }
    static END: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:^|\s)(/[^\s]*|@[^\s]*)$").unwrap());
    let Some(m) = END.captures(&before) else {
        return Ok((None, vec![]));
    };
    let tok = m.get(1).unwrap();
    let token = tok.as_str();
    let start = before[..tok.start()].chars().count();
    let choices: Vec<(String, String)> = if token.starts_with('/') {
        let head: String = chars[..start].iter().collect();
        if !head.trim().is_empty() {
            return Ok((None, vec![]));
        }
        ctx.commands()
            .map(|(n, d, u, _)| (n.to_string(), if u.is_empty() { d.to_string() } else { format!("{d} · {u}") }))
            .chain(ctx.generated.iter().map(|a| {
                (
                    a.command.clone(),
                    if a.usage.is_empty() {
                        a.description.clone()
                    } else {
                        format!("{} · {}", a.description, a.usage)
                    },
                )
            }))
            .collect()
    } else if token.to_lowercase().starts_with("@task:") {
        ctx.active_tasks()?
            .into_iter()
            .map(|t| (format!("@task:{}", t["id"]), s(&t["title"])))
            .collect()
    } else {
        TAGS.iter().map(|(n, d)| (n.to_string(), d.to_string())).collect()
    };
    let token = token.to_lowercase();
    Ok((
        Some((start, cursor)),
        choices.into_iter().filter(|(n, _)| n.starts_with(&token)).collect(),
    ))
}

/// The shared database's path for commands (read-only use).
pub fn default_context(generated: Vec<Generated>, personal: bool) -> Context {
    Context {
        db: crate::paths::db(),
        generated,
        personal,
    }
}
