//! Replays what the Python original recorded (rust/record/commands.py) against the port.
use std::cell::Cell;
use std::rc::Rc;

use serde_json::{json, Value};
use ultron_core::commands::{self, Context, Generated};
use ultron_core::config::Config;
use ultron_core::memory::Memory;
use ultron_core::router::{Ctx, Router, SkillError, Skills};

fn cases() -> Value {
    serde_json::from_str(include_str!("cases/commands.json")).unwrap()
}

fn same(a: &Value, b: &Value, path: &str) -> Result<(), String> {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            let (x, y) = (x.as_f64().unwrap(), y.as_f64().unwrap());
            if (x - y).abs() <= 1e-6 * x.abs().max(y.abs()).max(1.0) {
                Ok(())
            } else {
                Err(format!("{path}: {x} != {y}"))
            }
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => x
            .iter()
            .zip(y)
            .enumerate()
            .try_for_each(|(i, (p, q))| same(p, q, &format!("{path}[{i}]"))),
        (Value::Object(x), Value::Object(y)) if x.len() == y.len() => x
            .iter()
            .try_for_each(|(k, v)| same(v, y.get(k).ok_or(format!("{path}.{k} missing"))?, &format!("{path}.{k}"))),
        _ if a == b => Ok(()),
        _ => Err(format!("{path}:\n  got  {a}\n  want {b}")),
    }
}

fn context(all: &Value) -> (tempfile::TempDir, Context) {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("commands.db");
    std::fs::copy(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/cases/commands.db"), &db).unwrap();
    let generated: Vec<Generated> = serde_json::from_value(all["generated"].clone()).unwrap();
    (
        dir,
        Context {
            db,
            generated,
            personal: true,
        },
    )
}

#[test]
fn prepare_matches() {
    let all = cases();
    let (_dir, ctx) = context(&all);
    let mut bad = Vec::new();
    for c in all["prepare"].as_array().unwrap() {
        let got = match commands::prepare(c["args"][0].as_str().unwrap(), &ctx) {
            Ok(r) => json!({ "ok": r }),
            Err(e) => json!({ "err": e }),
        };
        if let Err(e) = same(&got, &c["out"], &format!("prepare({})", c["args"][0])) {
            bad.push(e);
        }
    }
    assert!(bad.is_empty(), "{} mismatches:\n{}", bad.len(), bad.join("\n"));
}

#[test]
fn suggestions_match() {
    let all = cases();
    let (_dir, ctx) = context(&all);
    let mut bad = Vec::new();
    for c in all["suggestions"].as_array().unwrap() {
        let text = c["args"][0].as_str().unwrap();
        let cursor = c["args"][1].as_u64().map(|n| n as usize);
        let (span, items) = commands::suggestions(text, cursor, &ctx).unwrap();
        let got = json!([span.map(|(a, b)| json!([a, b])), items]);
        if let Err(e) = same(&got, &c["out"], &format!("suggestions{}", c["args"])) {
            bad.push(e);
        }
    }
    assert!(bad.is_empty(), "{} mismatches:\n{}", bad.len(), bad.join("\n"));
}

struct Stubs;
impl Skills for Stubs {
    fn run(&mut self, name: &str, text: &str, _ctx: &mut Ctx) -> Result<String, SkillError> {
        if text.contains("UNCLEAR") {
            return Err(SkillError::Unclear(format!("{name} could not make sense of it")));
        }
        Ok(format!("{name} ran: {text}"))
    }
}

#[test]
fn handle_transcript_matches() {
    let all = cases();
    let (dir, cmds) = context(&all);
    let clock = Rc::new(Cell::new(1_790_000_000.0));
    let c2 = clock.clone();
    let tick = move || {
        c2.set(c2.get() + 1.0);
        c2.get()
    };
    let mem = Memory::open(Some(&dir.path().join("h.db")), tick).unwrap();
    let no_env = |_: &str| None;
    let cfg = Config {
        data_dir: dir.path().to_path_buf(),
        gemini: dir.path().join("gemini.json"),
        env: &no_env,
    };
    let mut brain = Router::new(Some(mem), None);
    brain.perf_ms = Box::new(|| 1000.0);
    let mut bad = Vec::new();
    for c in all["handle"].as_array().unwrap() {
        let mut ctx = Ctx {
            dry_run: true,
            ..Ctx::default()
        };
        let (reply, skill, conf) = brain.handle(c["args"][0].as_str().unwrap(), &cmds, &cfg, 7000.0, &mut Stubs, &mut ctx);
        let checks = [
            (json!([reply, skill, conf]), c["out"].clone(), "out"),
            (
                json!({"request": ctx.request, "mode": ctx.mode, "literal_title": ctx.literal_title,
                       "timer": ctx.timer.map(|(s, l)| json!([s, l])), "references": ctx.references}),
                c["ctx"].clone(),
                "ctx",
            ),
            (json!(brain.last.as_ref().map(|(t, at)| json!([t, at]))), c["last"].clone(), "last"),
        ];
        for (got, want, what) in checks {
            if let Err(e) = same(&got, &want, &format!("handle({}) {what}", c["args"][0])) {
                bad.push(e);
            }
        }
    }
    let conn = brain.memory.as_ref().unwrap().connection();
    let mut st = conn.prepare("SELECT text, route, ms FROM requests ORDER BY id").unwrap();
    let rows: Vec<Value> = st
        .query_map([], |r| {
            Ok(json!([r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, f64>(2)?]))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    if let Err(e) = same(&Value::Array(rows), &all["requests"], "requests log") {
        bad.push(e);
    }
    assert!(bad.is_empty(), "{} mismatches:\n{}", bad.len(), bad.join("\n"));
}

/// ADR-014: the founder's personal integrations are off in the public build.
#[test]
fn personal_commands_are_off_by_default() {
    let all = cases();
    let (_dir, mut ctx) = context(&all);
    ctx.personal = false;
    for cmd in [
        "/today",
        "/quest x",
        "/plan x",
        "/did x",
        "/log sleep 7h",
        "/dua",
        "/ayah",
        "/paper",
    ] {
        let r = commands::prepare(cmd, &ctx).unwrap();
        assert!(
            r.reply.as_deref().is_some_and(|m| m.starts_with("Unknown command")),
            "{cmd} should be unknown: {r:?}"
        );
    }
    let help = ctx.help();
    for gone in ["THE SYSTEM", "UmmahAPI", "Simora"] {
        assert!(!help.contains(gone), "help still mentions {gone}");
    }
    let (_, items) = commands::suggestions("/", None, &ctx).unwrap();
    assert!(items.iter().all(|(n, _)| !["/today", "/dua", "/paper"].contains(&n.as_str())));
    // everything else still works
    assert_eq!(commands::prepare("/timer 25 study", &ctx).unwrap().skill.as_deref(), Some("timer"));
}
