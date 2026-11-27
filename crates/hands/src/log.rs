//! The hands log, the guards and the self-report: ported from the original server.
//! Same file (`hands.log.jsonl`) and record shape, so the history carries over.
use std::collections::{HashMap, VecDeque};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use regex::Regex;
use serde_json::{json, Map, Value};

use crate::HandsError;

pub const MAX_ACTIONS_PER_MIN: usize = 120;

/// `$XDG_CACHE_HOME/clickhand` (default `~/.cache/clickhand`) on Linux, the platform cache dir elsewhere.
pub fn cache_dir() -> PathBuf {
    let base = if cfg!(target_os = "linux") {
        std::env::var_os("XDG_CACHE_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".cache"))
    } else {
        dirs::cache_dir().unwrap_or_default()
    };
    let dir = base.join("clickhand");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn log_path() -> PathBuf {
    cache_dir().join("hands.log.jsonl")
}

/// The kill switch: while this file exists, every input tool refuses.
pub fn stop_path() -> PathBuf {
    cache_dir().join("hands.stop")
}

pub fn readonly() -> bool {
    std::env::var("CLICKHAND_READONLY").as_deref() == Ok("1")
}

pub fn now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

pub fn write(mut rec: Map<String, Value>) {
    rec.insert("ts".into(), json!((now() * 1000.0).round() / 1000.0));
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log_path()) {
        let _ = writeln!(f, "{}", Value::Object(rec));
    }
}

static RECENT: LazyLock<Mutex<VecDeque<f64>>> = LazyLock::new(|| Mutex::new(VecDeque::new()));

/// Refuse input when read-only, when the kill switch is on, or past the rate limit.
pub fn guard(input_tool: bool) -> Result<(), HandsError> {
    if !input_tool {
        return Ok(());
    }
    if readonly() {
        return Err(HandsError::new(
            "Hands are in read-only mode (CLICKHAND_READONLY=1); input is disabled.",
        ));
    }
    let stop = stop_path();
    if stop.exists() {
        return Err(HandsError::new(format!(
            "Kill switch is on ({}). The user stopped the hands; do not retry — ask them.",
            stop.display()
        )));
    }
    let t = now();
    let mut recent = RECENT.lock().unwrap();
    while recent.front().is_some_and(|&f| t - f > 60.0) {
        recent.pop_front();
    }
    if recent.len() >= MAX_ACTIONS_PER_MIN {
        return Err(HandsError::new(
            "Rate limit: too many input actions in the last minute. Slow down and re-check the screen.",
        ));
    }
    recent.push_back(t);
    Ok(())
}

pub fn read(limit: usize) -> Vec<Value> {
    let Ok(text) = std::fs::read_to_string(log_path()) else {
        return vec![];
    };
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(limit)..]
        .iter()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

static DIGITS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d+").unwrap());
static SPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
static STAMP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[\d-]+ [\d:,]+ ").unwrap());

fn norm(msg: &str) -> String {
    SPACE.replace_all(&DIGITS.replace_all(msg, "N"), " ").chars().take(140).collect()
}

const SIGNATURES: &[(&str, &str)] = &[
    (
        r"(?i)windowactivate|no longer exists|take focus",
        "Stale/unfocusable window id — call list_windows right before focus_window.",
    ),
    (
        r"(?i)outside the screen",
        "Coordinate from an old screenshot — re-screenshot after any scroll/resize.",
    ),
    (
        r"(?i)Screen capture failed|scrot",
        "Screen capture failing — screen locked/asleep or capture unavailable.",
    ),
    (r"(?i)Kill switch", "User hit the kill switch — stop and ask."),
    (
        r"(?i)a11y|AT-SPI|no_app",
        "App exposes no accessibility tree — rely on screenshots/keyboard.",
    ),
    (r"(?i)timed out", "Command timeout — window/app hung or the screen is locked."),
    (
        r"(?i)Rate limit",
        "Too many actions — batch with keyboard shortcuts, look less often.",
    ),
];

/// Most common first; ties keep first-seen order, like Python's Counter.most_common.
fn top_counts(items: impl Iterator<Item = String>, n: usize) -> Vec<(String, usize)> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for it in items {
        if !counts.contains_key(&it) {
            order.push(it.clone());
        }
        *counts.entry(it).or_insert(0) += 1;
    }
    let mut v: Vec<(String, usize)> = order.into_iter().map(|k| (k.clone(), counts[&k])).collect();
    v.sort_by_key(|e| std::cmp::Reverse(e.1));
    v.into_iter().take(n).collect()
}

fn failed(r: &Value) -> bool {
    r["ok"] != json!(true)
}

/// Success rate per tool, recurring errors with fix hints, slow calls, feedback notes,
/// and the dominant failures in Clickhand's own log.
pub fn report(since_hours: f64) -> Value {
    let cutoff = now() - since_hours * 3600.0;
    let recs: Vec<Value> = read(5000)
        .into_iter()
        .filter(|r| r["ts"].as_f64().unwrap_or(0.0) >= cutoff)
        .collect();
    let calls: Vec<&Value> = recs.iter().filter(|r| r["tool"] != "feedback").collect();
    let mut order: Vec<String> = Vec::new();
    let mut stats: HashMap<String, (u64, u64, Vec<i64>)> = HashMap::new();
    for r in &calls {
        let t = r["tool"].as_str().unwrap_or("?").to_string();
        let s = stats.entry(t.clone()).or_insert_with(|| {
            order.push(t.clone());
            (0, 0, vec![])
        });
        s.0 += 1;
        s.1 += u64::from(failed(r));
        s.2.push(r["ms"].as_i64().unwrap_or(0));
    }
    let mut per = Map::new();
    for t in order {
        let (c, f, ms) = &stats[&t];
        let avg = ms.iter().sum::<i64>() / ms.len().max(1) as i64;
        per.insert(
            t,
            json!({"calls": c, "fail": f, "avg_ms": avg, "max_ms": ms.iter().max().copied().unwrap_or(0)}),
        );
    }
    let errs = top_counts(calls.iter().filter(|r| failed(r)).filter_map(|r| r["error"].as_str()).map(norm), 8);
    let top: Vec<Value> = errs
        .into_iter()
        .map(|(msg, c)| {
            let hint = SIGNATURES
                .iter()
                .find(|(p, _)| Regex::new(p).unwrap().is_match(&msg))
                .map(|(_, h)| *h);
            json!({"count": c, "error": msg, "hint": hint})
        })
        .collect();
    let text = std::fs::read_to_string(cache_dir().join("clickhand.log")).unwrap_or_default();
    let all: Vec<&str> = text.lines().collect();
    let tail = &all[all.len().saturating_sub(20000)..];
    let noisy = tail.iter().filter(|l| l.contains(" ERROR ") || l.contains(" WARNING "));
    let clickhand = top_counts(noisy.map(|l| norm(&STAMP.replace(l, ""))), 8);
    let mut slow: Vec<&&Value> = calls.iter().collect();
    slow.sort_by_key(|r| std::cmp::Reverse(r["ms"].as_i64().unwrap_or(0)));
    let feedback: Vec<&Value> = recs.iter().filter(|r| r["tool"] == "feedback").map(|r| &r["note"]).collect();
    json!({
        "window_hours": since_hours,
        "total_calls": calls.len(),
        "failures": calls.iter().filter(|r| failed(r)).count(),
        "per_tool": per,
        "top_errors": top,
        "slow_calls": slow.iter().take(5).map(|r| json!({"tool": r["tool"], "ms": r["ms"]})).collect::<Vec<_>>(),
        "feedback": feedback[feedback.len().saturating_sub(10)..].to_vec(),
        "clickhand_log_top": clickhand.into_iter().map(|(m, c)| json!({"count": c, "line": m})).collect::<Vec<_>>(),
    })
}
