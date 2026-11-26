//! Persistent task list and links to real focus sessions, ported from ultron/tasks.py.
//! Same tables and the same error messages as the original (they reach the dashboard).
//! `focus()` starts a timer on the island, so it lands with the UI (rust/MAP.md wave 3);
//! the database half of it is [`link_session`].
use chrono::{NaiveDate, Weekday};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{Map, Value};

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS tasks (
 id INTEGER PRIMARY KEY, title TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'todo',
 priority TEXT NOT NULL DEFAULT 'normal', due TEXT, created REAL NOT NULL,
 updated REAL NOT NULL, completed REAL
);
CREATE TABLE IF NOT EXISTS task_sessions (
 session_id INTEGER PRIMARY KEY REFERENCES sessions(id), task_id INTEGER NOT NULL REFERENCES tasks(id)
);
";

/// A task error, worded for the person reading the dashboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskError(pub String);

impl std::fmt::Display for TaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for TaskError {}

fn err<T>(msg: &str) -> Result<T, TaskError> {
    Err(TaskError(msg.to_string()))
}
fn db(e: rusqlite::Error) -> TaskError {
    TaskError(format!("Database error: {e}"))
}

pub fn ensure(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(SCHEMA)
}

/// Python's `date.fromisoformat` (3.11+): YYYY-MM-DD, YYYYMMDD, YYYY-Www[-D], YYYYWww[D].
pub fn parse_iso_date(s: &str) -> Option<NaiveDate> {
    let b = s.as_bytes();
    let digits = |r: std::ops::Range<usize>| -> Option<u32> {
        let part = b.get(r)?;
        if part.iter().all(u8::is_ascii_digit) {
            std::str::from_utf8(part).ok()?.parse().ok()
        } else {
            None
        }
    };
    let week = |year: u32, w: u32, d: u32| -> Option<NaiveDate> {
        let day = Weekday::try_from(u8::try_from(d.checked_sub(1)?).ok()?).ok()?;
        NaiveDate::from_isoywd_opt(year as i32, w, day)
    };
    match b.len() {
        10 if b[4] == b'-' && b[7] == b'-' => NaiveDate::from_ymd_opt(digits(0..4)? as i32, digits(5..7)?, digits(8..10)?),
        10 if b[4] == b'-' && b[5] == b'W' && b[8] == b'-' => week(digits(0..4)?, digits(6..8)?, digits(9..10)?),
        8 if b[4] == b'-' && b[5] == b'W' => week(digits(0..4)?, digits(6..8)?, 1),
        8 if b[4] == b'W' => week(digits(0..4)?, digits(5..7)?, digits(7..8)?),
        8 => NaiveDate::from_ymd_opt(digits(0..4)? as i32, digits(4..6)?, digits(6..8)?),
        7 if b[4] == b'W' => week(digits(0..4)?, digits(5..7)?, 1),
        _ => None,
    }
    .filter(|d| chrono::Datelike::year(d) >= 1) // Python's dates start at year 1
}

/// Python truthiness for JSON values.
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|x| x != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// Check and normalize task fields from the dashboard. `create` requires a title.
pub fn validate(fields: &Map<String, Value>, create: bool) -> Result<Map<String, Value>, TaskError> {
    if fields.keys().any(|k| !["title", "status", "priority", "due"].contains(&k.as_str())) {
        return err("Unknown task fields.");
    }
    let mut out = fields.clone();
    if create || out.contains_key("title") {
        match out.get("title").and_then(Value::as_str).map(str::trim) {
            Some(t) if (1..=240).contains(&t.chars().count()) => {
                let t = t.to_string();
                out.insert("title".into(), Value::String(t));
            }
            _ => return err("Give the task a title of 1–240 characters."),
        }
    }
    for (field, allowed) in [("status", ["todo", "doing", "done"]), ("priority", ["low", "normal", "high"])] {
        if let Some(v) = out.get(field) {
            if !v.as_str().is_some_and(|s| allowed.contains(&s)) {
                return err(&format!("Invalid task {field}."));
            }
        }
    }
    if let Some(due) = out.get("due").cloned() {
        if !truthy(&due) {
            out.insert("due".into(), Value::Null);
        } else {
            match due.as_str().and_then(parse_iso_date) {
                Some(d) => {
                    out.insert("due".into(), Value::String(d.to_string()));
                }
                None => return err("Use a valid date for the due date."),
            }
        }
    }
    Ok(out)
}

fn row_to_json(row: &rusqlite::Row<'_>) -> rusqlite::Result<Map<String, Value>> {
    use rusqlite::types::ValueRef;
    let mut m = Map::new();
    for (i, name) in row.as_ref().column_names().iter().enumerate() {
        let v = match row.get_ref(i)? {
            ValueRef::Null => Value::Null,
            ValueRef::Integer(n) => Value::from(n),
            ValueRef::Real(f) => Value::from(f),
            ValueRef::Text(t) => Value::String(String::from_utf8_lossy(t).into_owned()),
            ValueRef::Blob(_) => Value::Null,
        };
        m.insert(name.to_string(), v);
    }
    Ok(m)
}

pub fn get(conn: &Connection, task_id: i64) -> Result<Map<String, Value>, TaskError> {
    ensure(conn).map_err(db)?;
    conn.query_row("SELECT * FROM tasks WHERE id=?1", params![task_id], row_to_json)
        .optional()
        .map_err(db)?
        .ok_or_else(|| TaskError("Task not found.".into()))
}

pub fn create(conn: &Connection, fields: &Map<String, Value>, clock: &mut impl FnMut() -> f64) -> Result<Map<String, Value>, TaskError> {
    let f = validate(fields, true)?;
    ensure(conn).map_err(db)?;
    let now = clock();
    let status = f.get("status").and_then(Value::as_str).unwrap_or("todo");
    conn.execute(
        "INSERT INTO tasks(title,status,priority,due,created,updated,completed) VALUES(?1,?2,?3,?4,?5,?6,?7)",
        params![
            f["title"].as_str(),
            status,
            f.get("priority").and_then(Value::as_str).unwrap_or("normal"),
            f.get("due").and_then(Value::as_str),
            now,
            now,
            (status == "done").then_some(now),
        ],
    )
    .map_err(db)?;
    get(conn, conn.last_insert_rowid())
}

pub fn update(
    conn: &Connection,
    task_id: i64,
    fields: &Map<String, Value>,
    clock: &mut impl FnMut() -> f64,
) -> Result<Map<String, Value>, TaskError> {
    let mut f = validate(fields, false)?;
    get(conn, task_id)?;
    if let Some(status) = f.get("status").cloned() {
        let completed = if status.as_str() == Some("done") {
            Value::from(clock())
        } else {
            Value::Null
        };
        f.insert("completed".into(), completed);
    }
    f.insert("updated".into(), Value::from(clock()));
    let sets: Vec<String> = f.keys().enumerate().map(|(i, k)| format!("{k}=?{}", i + 1)).collect();
    let sql = format!("UPDATE tasks SET {} WHERE id=?{}", sets.join(","), f.len() + 1);
    let mut args: Vec<rusqlite::types::Value> = f
        .values()
        .map(|v| match v {
            Value::Null => rusqlite::types::Value::Null,
            Value::String(s) => rusqlite::types::Value::Text(s.clone()),
            Value::Number(n) => n
                .as_i64()
                .map(rusqlite::types::Value::Integer)
                .unwrap_or_else(|| rusqlite::types::Value::Real(n.as_f64().unwrap_or(0.0))),
            other => rusqlite::types::Value::Text(other.to_string()),
        })
        .collect();
    args.push(rusqlite::types::Value::Integer(task_id));
    conn.execute(&sql, rusqlite::params_from_iter(args)).map_err(db)?;
    get(conn, task_id)
}

/// Every task with its focused time and session count: doing, then todo, then done;
/// high priority first; earliest due date first; newest first.
pub fn listing(conn: &Connection) -> Result<Vec<Map<String, Value>>, TaskError> {
    ensure(conn).map_err(db)?;
    let mut stmt = conn
        .prepare(
            "SELECT t.*, COALESCE(SUM(s.active),0) AS focused,
      COUNT(s.id) AS sessions FROM tasks t LEFT JOIN task_sessions l ON l.task_id=t.id
      LEFT JOIN sessions s ON s.id=l.session_id GROUP BY t.id
      ORDER BY CASE t.status WHEN 'doing' THEN 0 WHEN 'todo' THEN 1 ELSE 2 END,
      CASE t.priority WHEN 'high' THEN 0 WHEN 'normal' THEN 1 ELSE 2 END,
      t.due IS NULL, t.due, t.created DESC",
        )
        .map_err(db)?;
    let rows = stmt.query_map([], row_to_json).map_err(db)?;
    rows.collect::<rusqlite::Result<_>>().map_err(db)
}

/// Count a timer session toward a task. False when that session already belongs to one.
pub fn link_session(conn: &Connection, session_id: i64, task_id: i64) -> Result<bool, TaskError> {
    ensure(conn).map_err(db)?;
    match conn.execute(
        "INSERT INTO task_sessions(session_id,task_id) VALUES(?1,?2)",
        params![session_id, task_id],
    ) {
        Ok(_) => Ok(true),
        Err(rusqlite::Error::SqliteFailure(e, _)) if e.code == rusqlite::ErrorCode::ConstraintViolation => Ok(false),
        Err(e) => Err(db(e)),
    }
}
