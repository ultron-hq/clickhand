//! Timer / stopwatch session log (SQLite), ported from ultron/sessions.py.
//!
//! A session is one timer or stopwatch run, from start until it finishes, is reset,
//! or Ultron exits. Each stretch of actual running (between pauses) is a *segment*;
//! reports sum segments, so pauses never count and a session crossing midnight
//! splits exactly. Durations come from a monotonic clock, so a suspended laptop
//! doesn't log the sleep. The schema is the original's, byte for byte (ADR-005).
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};

pub const HEARTBEAT_SECS: f64 = 30.0;
pub const UNTITLED: &str = "untitled";

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS sessions (
    id       INTEGER PRIMARY KEY,
    kind     TEXT NOT NULL,          -- timer | stopwatch
    label    TEXT NOT NULL,
    planned  REAL,                   -- timer length in s; NULL for stopwatch
    started  REAL NOT NULL,          -- unix time
    ended    REAL,
    active   REAL NOT NULL DEFAULT 0,-- running seconds, pauses excluded
    status   TEXT NOT NULL           -- open | completed | stopped | interrupted
);
CREATE TABLE IF NOT EXISTS segments (
    id         INTEGER PRIMARY KEY,
    session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    start      REAL NOT NULL,        -- unix time the stretch began
    secs       REAL NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS segments_start ON segments(start);
CREATE INDEX IF NOT EXISTS sessions_started ON sessions(started);
";

/// The shared database (see `paths`): the original's file on Linux.
pub fn default_db() -> PathBuf {
    crate::paths::db()
}

pub fn connect(path: Option<&Path>) -> rusqlite::Result<Connection> {
    let path = path.map(Path::to_path_buf).unwrap_or_else(default_db);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let conn = Connection::open(&path)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.pragma_update(None, "journal_mode", "WAL")?; // the GUI writes while the worker reads
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}

/// Collapse whitespace, trim punctuation, lowercase, keep 40 characters; empty → "untitled".
pub fn clean_label(label: Option<&str>) -> String {
    let joined = label.unwrap_or("").split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = joined.trim_matches(|c| " .,!?:;\"'".contains(c));
    let label: String = trimmed.to_lowercase().chars().take(40).collect();
    if label.is_empty() {
        UNTITLED.to_string()
    } else {
        label
    }
}

/// Rename the most recent session. Returns the stored label, or None when there are no sessions.
pub fn relabel_last(conn: &Connection, label: &str) -> rusqlite::Result<Option<String>> {
    let id: Option<i64> = conn
        .query_row("SELECT id FROM sessions ORDER BY started DESC LIMIT 1", [], |r| r.get(0))
        .optional()?;
    let Some(id) = id else { return Ok(None) };
    let label = clean_label(Some(label));
    conn.execute("UPDATE sessions SET label=?1 WHERE id=?2", params![label, id])?;
    Ok(Some(label))
}

/// Records the island's timer/stopwatch runs. `wall` is unix time, `mono` a monotonic clock in seconds.
pub struct Tracker<W: Fn() -> f64, M: Fn() -> f64> {
    conn: Option<Connection>,
    wall: W,
    mono: M,
    session: Option<i64>, // id of the open session (-1: timing without a database)
    segment: Option<i64>, // id of the running segment
    seg_wall: f64,
    seg_mono: f64,
    active: f64, // finished segments of the open session
    last_beat: f64,
}

impl Tracker<fn() -> f64, fn() -> f64> {
    /// A tracker on the real clocks.
    pub fn system(path: Option<&Path>) -> Self {
        fn wall() -> f64 {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0)
        }
        fn mono() -> f64 {
            use std::sync::OnceLock;
            static ORIGIN: OnceLock<std::time::Instant> = OnceLock::new();
            ORIGIN.get_or_init(std::time::Instant::now).elapsed().as_secs_f64()
        }
        Tracker::new(path, wall as fn() -> f64, mono as fn() -> f64)
    }
}

impl<W: Fn() -> f64, M: Fn() -> f64> Tracker<W, M> {
    pub fn new(path: Option<&Path>, wall: W, mono: M) -> Self {
        // Timers keep working without a database; nothing is recorded then.
        let conn = connect(path).and_then(|c| {
            c.execute(
                "UPDATE sessions SET status='interrupted', ended=started + active WHERE status='open'",
                [],
            )?;
            Ok(c)
        });
        Tracker {
            conn: conn.ok(),
            wall,
            mono,
            session: None,
            segment: None,
            seg_wall: 0.0,
            seg_mono: 0.0,
            active: 0.0,
            last_beat: 0.0,
        }
    }

    pub fn connection(&self) -> Option<&Connection> {
        self.conn.as_ref()
    }

    fn exec(&self, sql: &str, args: impl rusqlite::Params) -> Option<i64> {
        let conn = self.conn.as_ref()?;
        conn.execute(sql, args).ok()?;
        Some(conn.last_insert_rowid())
    }

    pub fn running(&self) -> bool {
        self.segment.is_some()
    }

    pub fn open_active(&self) -> f64 {
        self.active + if self.running() { (self.mono)() - self.seg_mono } else { 0.0 }
    }

    pub fn start(&mut self, kind: &str, label: Option<&str>, planned: Option<f64>) {
        if self.session.is_some() {
            self.finish("stopped");
        }
        let id = self.exec(
            "INSERT INTO sessions(kind, label, planned, started, status) VALUES (?1, ?2, ?3, ?4, 'open')",
            params![kind, clean_label(label), planned, (self.wall)()],
        );
        self.session = Some(id.unwrap_or(-1));
        self.active = 0.0;
        self.resume();
    }

    pub fn resume(&mut self) {
        let Some(session) = self.session else { return };
        if self.running() {
            return;
        }
        self.seg_wall = (self.wall)();
        self.seg_mono = (self.mono)();
        let id = self.exec(
            "INSERT INTO segments(session_id, start) VALUES (?1, ?2)",
            params![session, self.seg_wall],
        );
        self.segment = Some(id.unwrap_or(-1));
        self.last_beat = self.seg_mono;
    }

    pub fn pause(&mut self) {
        let Some(segment) = self.segment else { return };
        let secs = (self.mono)() - self.seg_mono;
        self.exec("UPDATE segments SET secs=?1 WHERE id=?2", params![secs, segment]);
        self.active += secs;
        self.segment = None;
        self.exec("UPDATE sessions SET active=?1 WHERE id=?2", params![self.active, self.session]);
    }

    /// completed (timer ran out) | stopped (reset / replaced) | interrupted (quit).
    pub fn finish(&mut self, status: &str) {
        let Some(session) = self.session else { return };
        self.pause();
        self.exec(
            "UPDATE sessions SET status=?1, ended=?2, active=?3 WHERE id=?4",
            params![status, (self.wall)(), self.active, session],
        );
        self.session = None;
        self.active = 0.0;
    }

    pub fn set_planned(&mut self, planned: Option<f64>) {
        if let Some(session) = self.session {
            self.exec("UPDATE sessions SET planned=?1 WHERE id=?2", params![planned, session]);
        }
    }

    /// Persist progress of the running segment (call often; it throttles itself).
    pub fn heartbeat(&mut self) {
        let Some(segment) = self.segment else { return };
        if (self.mono)() - self.last_beat < HEARTBEAT_SECS {
            return;
        }
        self.last_beat = (self.mono)();
        let secs = self.last_beat - self.seg_mono;
        self.exec("UPDATE segments SET secs=?1 WHERE id=?2", params![secs, segment]);
        self.exec(
            "UPDATE sessions SET active=?1 WHERE id=?2",
            params![self.active + secs, self.session],
        );
    }

    pub fn close(&mut self) {
        self.finish("interrupted");
        self.conn = None;
    }
}

impl<W: Fn() -> f64, M: Fn() -> f64> Drop for Tracker<W, M> {
    fn drop(&mut self) {
        if self.conn.is_some() {
            self.close();
        }
    }
}
