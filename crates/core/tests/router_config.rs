//! Replays what the Python original recorded (rust/record/router_config.py) against the port.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use serde_json::{json, Map, Value};
use ultron_core::config::{self, Config};
use ultron_core::memory::Memory;
use ultron_core::router::{self, Ctx, Plan, Planner, Registry, Router, SkillError, Skills};

fn cases() -> Value {
    serde_json::from_str(include_str!("cases/router_config.json")).unwrap()
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
        _ => Err(format!("{path}: got {a} want {b}")),
    }
}

#[test]
fn rules_corrections_and_questions_match() {
    let all = cases();
    let r = &all["router"];
    let reg = Registry::builtin();
    let mut bad = Vec::new();
    for c in r["rule"].as_array().unwrap() {
        let t = c["args"][0].as_str().unwrap();
        if json!(reg.rule(t)) != c["out"] {
            bad.push(format!("rule({t:?}) = {:?}, want {}", reg.rule(t), c["out"]));
        }
    }
    for c in r["correction"].as_array().unwrap() {
        let t = c["args"][0].as_str().unwrap();
        if json!(router::correction(t)) != c["out"] {
            bad.push(format!("correction({t:?}) = {:?}, want {}", router::correction(t), c["out"]));
        }
    }
    for c in r["question"].as_array().unwrap() {
        let t = c["args"][0].as_str().unwrap();
        if json!(router::is_question(t)) != c["out"] {
            bad.push(format!("is_question({t:?}), want {}", c["out"]));
        }
    }
    assert!(bad.is_empty(), "{} mismatches:\n{}", bad.len(), bad.join("\n"));
}

/// Echo skills, like the recorder's stubs.
struct Stubs;
impl Skills for Stubs {
    fn run(&mut self, name: &str, text: &str, _ctx: &mut Ctx) -> Result<String, SkillError> {
        if text.contains("UNCLEAR") {
            return Err(SkillError::Unclear(format!("{name} could not make sense of it")));
        }
        if text.contains("CRASH") {
            return Err(SkillError::Failed("boom".into()));
        }
        Ok(format!("{name} ran: {text}"))
    }
}

struct Scripted {
    calls: Rc<RefCell<Vec<Value>>>,
}
impl Planner for Scripted {
    fn plan(&mut self, text: &str, notes: &[String]) -> Option<Plan> {
        self.calls.borrow_mut().push(json!([text, notes]));
        let run = |s: &str, c: &str| {
            Some(Plan::Run {
                skill: s.into(),
                command: c.into(),
            })
        };
        match text {
            "show me some cat videos" => run("web_search", "search youtube for cat videos"),
            "what's the capital of peru" => Some(Plan::Answer("Lima.".into())),
            "put on something relaxing" => run("open_app", "open spotify"),
            "make it UNCLEAR please" => run("open_app", "open UNCLEAR thing"),
            "tell me about my locker" => Some(Plan::Answer("Your locker is 42.".into())),
            "track my focus" => run("time_report", "time report today"),
            _ => None,
        }
    }
}

#[test]
fn routing_transcript_matches() {
    let all = cases();
    let r = &all["router"];
    let dir = tempfile::tempdir().unwrap();
    let clock = Rc::new(Cell::new(1_790_000_000.0));
    let c2 = clock.clone();
    let tick = move || {
        c2.set(c2.get() + 1.0);
        c2.get()
    };
    let mut mem = Memory::open(Some(&dir.path().join("r.db")), tick).unwrap();
    mem.remember("my locker is number 42 at the gym").unwrap();
    mem.add_alias("my music", "spotify").unwrap();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut brain = Router::new(Some(mem), Some(Box::new(Scripted { calls: calls.clone() })));
    let mono = 5000.0;
    let mut bad = Vec::new();
    for c in r["route"].as_array().unwrap() {
        let op = c["op"].as_array().unwrap();
        if op[0] == "last" {
            brain.last = Some((op[1].as_str().unwrap().into(), mono - op[2].as_f64().unwrap()));
            continue;
        }
        let mut ctx = Ctx {
            dry_run: true,
            ..Ctx::default()
        };
        let text = op[1].as_str().unwrap();
        let res = if op[0] == "correct" {
            brain.correct(text, mono, &mut Stubs, &mut ctx)
        } else {
            Some(brain.route(text, &mut Stubs, &mut ctx))
        };
        let got = res.map(|o| {
            json!({"reply": o.reply, "skill": o.skill, "conf": o.conf, "via": o.via, "command": o.command, "ok": o.ok, "origin": ctx.origin.as_str()})
        });
        if let Err(e) = same(&json!(got), &c["out"], &format!("{}", c["op"])) {
            bad.push(e);
        }
        let last = brain.last.as_ref().map(|(t, at)| json!([t, at]));
        if let Err(e) = same(&json!(last), &c["last"], &format!("last after {}", c["op"])) {
            bad.push(e);
        }
    }
    if let Err(e) = same(&Value::Array(calls.borrow().clone()), &r["planner_calls"], "planner calls") {
        bad.push(e);
    }
    let conn = brain.memory.as_ref().unwrap().connection();
    for table in ["learned", "facts", "aliases"] {
        let mut st = conn.prepare(&format!("SELECT * FROM {table} ORDER BY 1")).unwrap();
        let n = st.column_count();
        let rows: Vec<Value> = st
            .query_map([], |row| {
                Ok(Value::Array(
                    (0..n)
                        .map(|i| match row.get_ref(i).unwrap() {
                            rusqlite::types::ValueRef::Null => Value::Null,
                            rusqlite::types::ValueRef::Integer(v) => json!(v),
                            rusqlite::types::ValueRef::Real(v) => json!(v),
                            rusqlite::types::ValueRef::Text(t) => json!(String::from_utf8_lossy(t)),
                            rusqlite::types::ValueRef::Blob(_) => Value::Null,
                        })
                        .collect(),
                ))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect();
        if let Err(e) = same(&Value::Array(rows), &r["tables"][table], table) {
            bad.push(e);
        }
    }
    assert!(bad.is_empty(), "{} mismatches:\n{}", bad.len(), bad.join("\n"));
}

fn file(p: &std::path::Path) -> Value {
    match std::fs::read_to_string(p) {
        Err(_) => Value::Null,
        Ok(t) => serde_json::from_str(&t).unwrap_or_else(|_| json!({ "raw": t })),
    }
}

#[test]
fn config_matches() {
    let all = cases();
    let cfgcases = &all["config"];
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().to_path_buf();
    let env: RefCell<Map<String, Value>> = RefCell::new(Map::new());
    let lookup = |k: &str| env.borrow().get(k).and_then(|v| v.as_str()).map(str::to_string);
    let cfg = Config {
        data_dir: d.clone(),
        gemini: d.join("gemini.json"),
        env: &lookup,
    };
    let prefs = d.join("preferences.json");
    let mut bad = Vec::new();

    // preferences: the recorder rewrote the file before some reads
    let setups: [Option<&str>; 5] = [
        None,
        Some("not json"),
        Some(r#"{"model": "opus", "junk": 1, "effort": "high"}"#),
        Some(r#"["a", "list"]"#),
        None,
    ];
    for (i, c) in cfgcases["preferences"].as_array().unwrap().iter().enumerate() {
        if i < setups.len() {
            if let Some(body) = setups[i] {
                std::fs::write(&prefs, body).unwrap();
            }
        }
        if i == 4 {
            let _ = std::fs::remove_file(&prefs);
        }
        let op = c["op"].as_array().unwrap();
        let got = if op[0] == "read" {
            json!({ "ok": cfg.read() })
        } else {
            match cfg.save(op[1].as_object().unwrap()) {
                Ok(m) => json!({ "ok": m }),
                Err(e) => json!({ "err": e }),
            }
        };
        if let Err(e) = same(&got, &c["out"], &format!("preferences {}", c["op"])) {
            bad.push(e);
        }
        if let Err(e) = same(&file(&prefs), &c["file"], &format!("preferences file after {}", c["op"])) {
            bad.push(e);
        }
    }

    // Gemini settings
    for c in cfgcases["live_config"].as_array().unwrap() {
        let op = c["op"].as_array().unwrap();
        *env.borrow_mut() = op[3].as_object().cloned().unwrap_or_default();
        // the recorder replaced the file with a JSON list before its 14th step
        if c == &cfgcases["live_config"][13] {
            std::fs::write(&cfg.gemini, "[1,2]").unwrap();
        }
        let got = if op[0] == "read" {
            json!({ "ok": cfg.gemini() })
        } else {
            let a = op[1].as_array().unwrap();
            let kw = op[2].as_object().unwrap();
            let model = kw.get("model").and_then(Value::as_str).unwrap_or(config::DEFAULT_GEMINI_MODEL);
            let voice = kw.get("voice").and_then(Value::as_str).unwrap_or(config::DEFAULT_VOICE);
            match cfg.save_gemini(a[0].as_str().unwrap(), model, voice) {
                Ok(()) => json!({ "ok": null }),
                Err(e) => json!({ "err": e }),
            }
        };
        if let Err(e) = same(&got, &c["out"], &format!("live {}", c["op"])) {
            bad.push(e);
        }
        if let Err(e) = same(&file(&cfg.gemini), &c["file"], &format!("gemini file after {}", c["op"])) {
            bad.push(e);
        }
    }

    // model settings
    env.borrow_mut().clear();
    let _ = std::fs::remove_file(&prefs);
    let mut set_local = false;
    for c in cfgcases["model_settings"].as_array().unwrap() {
        let a = c["args"].as_array().unwrap();
        let got = match c["fn"].as_str().unwrap() {
            "is_command" => json!(config::is_command(a[0].as_str().unwrap())),
            "handle" => json!(cfg.handle(a[0].as_str().unwrap(), a[1].as_bool().unwrap()).unwrap()),
            "status" => {
                if !set_local {
                    let mut v = Map::new();
                    v.insert("voice_provider".into(), json!("local"));
                    cfg.save(&v).unwrap();
                    set_local = true;
                }
                *env.borrow_mut() = c.get("env").and_then(Value::as_object).cloned().unwrap_or_default();
                json!(cfg.status(None))
            }
            "claude_options" => json!(config::claude_options(a[0].as_str().unwrap(), a[1].as_str().unwrap())),
            other => panic!("unknown fn {other}"),
        };
        if let Err(e) = same(&got, &c["out"], &format!("{}{}", c["fn"], c["args"])) {
            bad.push(e);
        }
    }
    assert!(bad.is_empty(), "{} mismatches:\n{}", bad.len(), bad.join("\n"));
}
