//! Replays what the Python original recorded (rust/record/sessions_reports.py) against the port.
//! Every case must match: strings exactly, numbers to within float noise.
use std::cell::Cell;

use chrono::NaiveDate;
use chrono_tz::Europe::Paris;
use serde_json::Value;
use ultron_core::{reports, sessions};

fn cases() -> Value {
    serde_json::from_str(include_str!("cases/sessions_reports.json")).unwrap()
}

fn fixture() -> (tempfile::TempDir, rusqlite::Connection) {
    // work on a copy, so the recorded database never changes
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("reports.db");
    std::fs::copy(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/cases/reports.db"), &path).unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    (dir, conn)
}

fn date(v: &Value) -> NaiveDate {
    v.as_str().unwrap().parse().unwrap()
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
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                return Err(format!("{path}: length {} != {}", x.len(), y.len()));
            }
            x.iter()
                .zip(y)
                .enumerate()
                .try_for_each(|(i, (p, q))| same(p, q, &format!("{path}[{i}]")))
        }
        (Value::Object(x), Value::Object(y)) => {
            if x.len() != y.len() {
                return Err(format!(
                    "{path}: keys {:?} != {:?}",
                    x.keys().collect::<Vec<_>>(),
                    y.keys().collect::<Vec<_>>()
                ));
            }
            x.iter()
                .try_for_each(|(k, v)| same(v, y.get(k).ok_or(format!("{path}.{k} missing"))?, &format!("{path}.{k}")))
        }
        _ if a == b => Ok(()),
        _ => Err(format!("{path}: {a} != {b}")),
    }
}

#[test]
fn fmt_matches() {
    for c in cases()["fmt"].as_array().unwrap() {
        assert_eq!(
            reports::fmt(c["args"][0].as_f64().unwrap()),
            c["out"].as_str().unwrap(),
            "fmt({})",
            c["args"][0]
        );
    }
}

#[test]
fn clean_label_matches() {
    for c in cases()["clean_label"].as_array().unwrap() {
        assert_eq!(
            sessions::clean_label(c["args"][0].as_str()),
            c["out"].as_str().unwrap(),
            "clean_label({})",
            c["args"][0]
        );
    }
}

#[test]
fn bounds_and_edges_match() {
    let all = cases();
    for c in all["bounds"].as_array().unwrap() {
        let (a, b, unit) = reports::bounds(c["args"][0].as_str().unwrap(), date(&c["args"][1])).unwrap();
        let got = serde_json::json!([a.to_string(), b.to_string(), unit]);
        same(&got, &c["out"], &format!("bounds{}", c["args"])).unwrap();
    }
    for c in all["edges"].as_array().unwrap() {
        let unit: reports::Unit = match c["args"][2].as_str().unwrap() {
            "hour" => reports::Unit::Hour,
            "day" => reports::Unit::Day,
            _ => reports::Unit::Month,
        };
        let (e, names) = reports::edges(&Paris, date(&c["args"][0]), date(&c["args"][1]), unit);
        same(&serde_json::json!([e, names]), &c["out"], &format!("edges{}", c["args"])).unwrap();
    }
}

#[test]
fn summaries_and_one_liners_match() {
    let (_dir, conn) = fixture();
    let all = cases();
    let mut failures = Vec::new();
    for (c, line) in all["summarize"]
        .as_array()
        .unwrap()
        .iter()
        .zip(all["one_liner"].as_array().unwrap())
    {
        let period = c["args"][0].as_str().unwrap();
        let label = c["args"][1].as_str();
        let s = reports::summarize(&conn, &Paris, period, label, date(&c["args"][2])).unwrap();
        if let Err(e) = same(&serde_json::to_value(&s).unwrap(), &c["out"], &format!("summarize{}", c["args"])) {
            failures.push(e);
        }
        if reports::one_liner(&s) != line["out"].as_str().unwrap() {
            failures.push(format!("one_liner{}: {:?} != {}", c["args"], reports::one_liner(&s), line["out"]));
        }
    }
    assert!(
        failures.is_empty(),
        "{} mismatches, first:\n{}",
        failures.len(),
        failures.iter().take(5).cloned().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn streaks_day_totals_and_labels_match() {
    let (_dir, conn) = fixture();
    let all = cases();
    for c in all["streak"].as_array().unwrap() {
        assert_eq!(
            reports::streak(&conn, &Paris, date(&c["args"][0])).unwrap() as u64,
            c["out"].as_u64().unwrap(),
            "streak{}",
            c["args"]
        );
    }
    for c in all["day_totals"].as_array().unwrap() {
        let got: serde_json::Map<String, Value> = reports::day_totals(&conn, &Paris, date(&c["args"][0]), date(&c["args"][1]))
            .unwrap()
            .into_iter()
            .map(|(d, v)| (d.to_string(), serde_json::json!(v)))
            .collect();
        same(&Value::Object(got), &c["out"], &format!("day_totals{}", c["args"])).unwrap();
    }
    let got = reports::known_labels(&conn).unwrap();
    assert_eq!(serde_json::json!(got), all["known_labels"]);
}

#[test]
fn relabel_last_matches() {
    let (_dir, conn) = fixture();
    let c = &cases()["relabel_last"];
    let out = sessions::relabel_last(&conn, c["args"][0].as_str().unwrap()).unwrap();
    assert_eq!(serde_json::json!(out), c["out"]);
    let after: String = conn
        .query_row("SELECT label FROM sessions ORDER BY started DESC LIMIT 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(after, c["label_after"].as_str().unwrap());
}

#[test]
fn tracker_writes_the_same_rows() {
    let all = cases();
    let c = &all["tracker"];
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tracker.db");
    let wall = Cell::new(c["start"]["wall"].as_f64().unwrap());
    let mono = Cell::new(c["start"]["mono"].as_f64().unwrap());
    {
        let mut tr = sessions::Tracker::new(Some(&path), || wall.get(), || mono.get());
        for op in c["script"].as_array().unwrap() {
            match op[0].as_str().unwrap() {
                "tick" => {
                    let d = op[1].as_f64().unwrap();
                    wall.set(wall.get() + d);
                    mono.set(mono.get() + d);
                }
                "start" => tr.start(op[1].as_str().unwrap(), op[2].as_str(), op[3].as_f64()),
                "finish" => tr.finish(op[1].as_str().unwrap()),
                "set_planned" => tr.set_planned(op[1].as_f64()),
                "pause" => tr.pause(),
                "resume" => tr.resume(),
                "heartbeat" => tr.heartbeat(),
                "close" => tr.close(),
                other => panic!("unknown op {other}"),
            }
        }
    }
    let conn = rusqlite::Connection::open(&path).unwrap();
    let rows = |sql: &str, n: usize| -> Value {
        let mut st = conn.prepare(sql).unwrap();
        let out: Vec<Value> = st
            .query_map([], |r| {
                Ok(Value::Array(
                    (0..n)
                        .map(|i| match r.get_ref(i).unwrap() {
                            rusqlite::types::ValueRef::Null => Value::Null,
                            rusqlite::types::ValueRef::Integer(v) => serde_json::json!(v),
                            rusqlite::types::ValueRef::Real(v) => serde_json::json!(v),
                            rusqlite::types::ValueRef::Text(t) => serde_json::json!(String::from_utf8_lossy(t)),
                            rusqlite::types::ValueRef::Blob(_) => Value::Null,
                        })
                        .collect(),
                ))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect();
        Value::Array(out)
    };
    same(
        &rows(
            "SELECT id,kind,label,planned,started,ended,active,status FROM sessions ORDER BY id",
            8,
        ),
        &c["sessions"],
        "sessions",
    )
    .unwrap();
    same(
        &rows("SELECT id,session_id,start,secs FROM segments ORDER BY id", 4),
        &c["segments"],
        "segments",
    )
    .unwrap();
}
