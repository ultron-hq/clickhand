//! Replays what the Python original recorded (rust/record/memory_tasks.py) against the port.
use std::cell::Cell;

use serde_json::{json, Map, Value};
use ultron_core::{memory, memory::Change, sessions, tasks};

fn cases() -> Value {
    serde_json::from_str(include_str!("cases/memory_tasks.json")).unwrap()
}

/// Same JSON, numbers equal to within 1e-6 relative.
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
fn normalize_words_and_templates_match() {
    let all = cases();
    let m = &all["memory"];
    for c in m["normalize"].as_array().unwrap() {
        assert_eq!(
            memory::normalize(c["args"][0].as_str().unwrap()),
            c["out"].as_str().unwrap(),
            "normalize({})",
            c["args"][0]
        );
    }
    for c in m["words"].as_array().unwrap() {
        assert_eq!(
            json!(memory::words(c["args"][0].as_str().unwrap())),
            c["out"],
            "words({})",
            c["args"][0]
        );
    }
    for c in m["build_template"].as_array().unwrap() {
        let got = memory::build_template(c["args"][0].as_str().unwrap(), c["args"][1].as_str().unwrap()).map(|(a, b)| vec![a, b]);
        assert_eq!(json!(got), c["out"], "build_template{}", c["args"]);
    }
}

fn change(c: &Option<Change>) -> Value {
    match c {
        None => Value::Null,
        Some(Change::Fact(i)) => json!(["fact", i]),
        Some(Change::Alias(i)) => json!(["alias", i]),
        Some(Change::Learned(i)) => json!(["learned", i]),
    }
}

#[test]
fn memory_script_matches() {
    let all = cases();
    let m = &all["memory"];
    let dir = tempfile::tempdir().unwrap();
    let clock = Cell::new(1_790_000_000.0);
    let tick = || {
        clock.set(clock.get() + 1.0);
        clock.get()
    };
    let mut mem = memory::Memory::open(Some(&dir.path().join("m.db")), tick).unwrap();
    let s = |v: &Value| v.as_str().unwrap().to_string();
    let mut failures = Vec::new();
    for c in m["ops"].as_array().unwrap() {
        let op = c["op"].as_array().unwrap();
        let a = &op[1..];
        let got: Value = match op[0].as_str().unwrap() {
            "tick" => {
                clock.set(clock.get() + a[0].as_f64().unwrap());
                Value::Null
            }
            "remember" => json!(mem.remember(&s(&a[0])).unwrap()),
            "facts_about" => json!(mem.facts_about(&s(&a[0]), a[1].as_u64().unwrap() as usize).unwrap()),
            "recall" => json!(mem.recall(&s(&a[0])).unwrap()),
            "add_alias" => json!(mem.add_alias(&s(&a[0]), &s(&a[1])).unwrap()),
            "expand" => json!(mem.expand(&s(&a[0])).unwrap()),
            "learn" => {
                mem.learn(&s(&a[0]), &s(&a[1]), a[2].as_str(), a[3].as_str(), &s(&a[4])).unwrap();
                Value::Null
            }
            "lookup" => json!(mem.lookup(&s(&a[0])).unwrap()),
            "used" => {
                mem.used(a[0].as_i64().unwrap()).unwrap();
                Value::Null
            }
            "failed" => {
                mem.failed(a[0].as_i64().unwrap()).unwrap();
                Value::Null
            }
            "counts" => json!(mem.counts().unwrap()),
            "log_request" => {
                mem.log_request(&s(&a[0]), &s(&a[1]), a[2].as_f64());
                Value::Null
            }
            "routes" => json!(mem.routes(a[0].as_f64().unwrap()).unwrap()),
            "set_spot" => json!(mem.set_spot(&s(&a[0]), a[1].as_f64().unwrap(), a[2].as_f64().unwrap()).unwrap()),
            "get_spot" => json!(mem.get_spot(&s(&a[0])).unwrap()),
            "spots" => json!(mem.spots().unwrap()),
            "forget_spot" => json!(mem.forget_spot(&s(&a[0])).unwrap()),
            "facts" => json!(mem.facts().unwrap()),
            "aliases" => json!(mem.aliases().unwrap()),
            "forget" => json!(mem.forget(&s(&a[0])).unwrap()),
            "forget_everything" => json!(mem.forget_everything().unwrap()),
            other => panic!("unknown op {other}"),
        };
        if let Err(e) = same(&got, &c["out"], &format!("{}", c["op"])) {
            failures.push(e);
        }
        if op[0] != "tick" {
            if let Err(e) = same(
                &change(&mem.last_change),
                &c["last_change"],
                &format!("last_change after {}", c["op"]),
            ) {
                failures.push(e);
            }
        }
    }
    for (table, cols) in [
        ("facts", "id,text,norm,created"),
        ("aliases", "id,phrase,meaning,created"),
        (
            "learned",
            "id,kind,key,skill,command,answer,source,example,uses,fails,created,last_used",
        ),
        ("spots", "name,x,y,created"),
        ("requests", "id,at,text,route,ms"),
    ] {
        let conn = mem.connection();
        let mut st = conn.prepare(&format!("SELECT {cols} FROM {table} ORDER BY 1")).unwrap();
        let n = st.column_count();
        let rows: Vec<Value> = st
            .query_map([], |r| {
                Ok(Value::Array(
                    (0..n)
                        .map(|i| match r.get_ref(i).unwrap() {
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
        if let Err(e) = same(&Value::Array(rows), &m["tables"][table], table) {
            failures.push(e);
        }
    }
    assert!(failures.is_empty(), "{} mismatches:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn task_validation_matches() {
    for c in cases()["tasks"]["validate"].as_array().unwrap() {
        let fields = c["args"][0].as_object().unwrap();
        let got = match tasks::validate(fields, c["args"][1].as_bool().unwrap()) {
            Ok(m) => json!({ "ok": m }),
            Err(e) => json!({ "err": e.0 }),
        };
        same(&got, &c["out"], &format!("validate{}", c["args"])).unwrap();
    }
}

#[test]
fn task_script_matches() {
    let all = cases();
    let dir = tempfile::tempdir().unwrap();
    let conn = sessions::connect(Some(&dir.path().join("t.db"))).unwrap();
    for (label, active) in [("write report", 1500.0), ("study", 3000.0), ("write report", 900.0)] {
        conn.execute(
            "INSERT INTO sessions(kind,label,planned,started,active,status) VALUES ('timer',?1,1500,1790000000,?2,'completed')",
            rusqlite::params![label, active],
        )
        .unwrap();
    }
    let mut t = 1_790_000_000.0;
    let mut clock = || {
        t += 1.0;
        t
    };
    for c in all["tasks"]["ops"].as_array().unwrap() {
        let op = c["op"].as_array().unwrap();
        let obj = |v: &Value| v.as_object().cloned().unwrap_or_else(Map::new);
        let res: Result<Value, tasks::TaskError> = match op[0].as_str().unwrap() {
            "create" => tasks::create(&conn, &obj(&op[1]), &mut clock).map(Value::Object),
            "update" => tasks::update(&conn, op[1].as_i64().unwrap(), &obj(&op[2]), &mut clock).map(Value::Object),
            "get" => tasks::get(&conn, op[1].as_i64().unwrap()).map(Value::Object),
            "listing" => tasks::listing(&conn).map(|l| json!(l)),
            "link" => tasks::link_session(&conn, op[2].as_i64().unwrap(), op[1].as_i64().unwrap())
                .map(|ok| json!(if ok { "linked" } else { "already linked" })),
            other => panic!("unknown op {other}"),
        };
        let got = match res {
            Ok(v) => json!({ "ok": v }),
            Err(e) => json!({ "err": e.0 }),
        };
        same(&got, &c["out"], &format!("{}", c["op"])).unwrap();
    }
}
