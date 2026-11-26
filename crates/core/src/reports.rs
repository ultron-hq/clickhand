//! Time reports from the session log, ported from ultron/reports.py: period bounds,
//! buckets, streaks, summaries and the island's one-liners. (The HTML dashboard
//! comes with the tasks/runs/abilities units.)
//!
//! Every function takes the time zone explicitly, so results don't depend on the
//! machine's TZ setting, and reproduces Python's local-time rules: a wall time that
//! falls in a DST gap or repeats in a DST overlap resolves the way
//! `datetime.timestamp()` resolves it (fold 0 unless it came from `fromtimestamp`).
use chrono::{Datelike, Duration, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, Offset, TimeZone, Timelike};
use indexmap::IndexMap;
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::sessions::UNTITLED;

pub const MIN_SECS: f64 = 5.0; // shorter runs are accidental clicks, not sessions
pub const STREAK_SECS: f64 = 60.0; // a day counts toward the streak with at least this much
pub const PERIODS: [&str; 7] = ["today", "yesterday", "week", "last week", "month", "last month", "year"];

pub fn title(period: &str) -> &'static str {
    match period {
        "today" => "Today",
        "yesterday" => "Yesterday",
        "week" => "This week",
        "last week" => "Last week",
        "month" => "This month",
        "last month" => "Last month",
        _ => "This year",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Unit {
    Hour,
    Day,
    Month,
}

// ── local time, the way Python does it ────────────────────────────────────────

/// Unix time of a local wall time. `fold` picks the second occurrence of a repeated
/// time; in a gap, fold 0 uses the offset from before the change (Python's rule).
pub fn local_ts<Tz: TimeZone>(tz: &Tz, naive: NaiveDateTime, fold: bool) -> f64 {
    match tz.from_local_datetime(&naive) {
        LocalResult::Single(t) => t.timestamp() as f64,
        LocalResult::Ambiguous(a, b) => (if fold { b } else { a }).timestamp() as f64,
        LocalResult::None => {
            let probe = if fold {
                naive + Duration::hours(6)
            } else {
                naive - Duration::hours(6)
            };
            let off = match tz.from_local_datetime(&probe) {
                LocalResult::Single(t) | LocalResult::Ambiguous(t, _) => t.offset().fix().local_minus_utc(),
                LocalResult::None => 0,
            };
            (naive.and_utc().timestamp() - off as i64) as f64
        }
    }
}

/// Local midnight of a date as unix time (DST-correct).
pub fn ts<Tz: TimeZone>(tz: &Tz, d: NaiveDate) -> f64 {
    local_ts(tz, d.and_time(NaiveTime::MIN), false)
}

/// `datetime.fromtimestamp(t)`: local wall time, and whether it is the second occurrence.
fn from_ts<Tz: TimeZone>(tz: &Tz, t: f64) -> (NaiveDateTime, bool) {
    let secs = t.floor() as i64;
    let nanos = ((t - t.floor()) * 1e9).round() as u32;
    let dt = tz.timestamp_opt(secs, nanos.min(999_999_999)).single().expect("valid unix time");
    let naive = dt.naive_local();
    let fold = matches!(tz.from_local_datetime(&naive), LocalResult::Ambiguous(_, b) if b.timestamp() == secs);
    (naive, fold)
}

pub fn add_months(d: NaiveDate, n: i32) -> NaiveDate {
    let m = d.month0() as i32 + n;
    NaiveDate::from_ymd_opt(d.year() + m.div_euclid(12), m.rem_euclid(12) as u32 + 1, 1).unwrap()
}

/// (start date, end date exclusive, bucket unit) for a period name.
pub fn bounds(period: &str, today: NaiveDate) -> Option<(NaiveDate, NaiveDate, Unit)> {
    let t = today;
    let day = Duration::days(1);
    Some(match period {
        "today" => (t, t + day, Unit::Hour),
        "yesterday" => (t - day, t, Unit::Hour),
        "week" | "last week" => {
            let monday = t - Duration::days(t.weekday().num_days_from_monday() as i64);
            if period == "week" {
                (monday, monday + Duration::days(7), Unit::Day)
            } else {
                (monday - Duration::days(7), monday, Unit::Day)
            }
        }
        "month" | "last month" => {
            let first = t.with_day(1).unwrap();
            if period == "month" {
                (first, add_months(first, 1), Unit::Day)
            } else {
                (add_months(first, -1), first, Unit::Day)
            }
        }
        "year" => (
            NaiveDate::from_ymd_opt(t.year(), 1, 1)?,
            NaiveDate::from_ymd_opt(t.year() + 1, 1, 1)?,
            Unit::Month,
        ),
        _ => return None,
    })
}

/// Bucket boundaries (unix times) and labels between two dates.
pub fn edges<Tz: TimeZone>(tz: &Tz, start: NaiveDate, end: NaiveDate, unit: Unit) -> (Vec<f64>, Vec<String>) {
    let (mut out, mut names) = (Vec::new(), Vec::new());
    match unit {
        Unit::Hour => {
            let mut day = start;
            while day < end {
                for h in 0..24 {
                    out.push(local_ts(tz, day.and_hms_opt(h, 0, 0).unwrap(), false));
                    names.push(format!("{h:02}"));
                }
                day += Duration::days(1);
            }
        }
        Unit::Day => {
            let short = (end - start).num_days() <= 7;
            let mut d = start;
            while d < end {
                out.push(ts(tz, d));
                names.push(if short {
                    d.format("%a %d").to_string()
                } else {
                    d.day().to_string()
                });
                d += Duration::days(1);
            }
        }
        Unit::Month => {
            let mut d = start;
            while d < end {
                out.push(ts(tz, d));
                names.push(d.format("%b").to_string());
                d = add_months(d, 1);
            }
        }
    }
    out.push(ts(tz, end));
    (out, names)
}

/// Sum (start, end) intervals into the buckets delimited by `edge`.
pub fn spread(pieces: &[(f64, f64)], edge: &[f64]) -> Vec<f64> {
    let mut totals = vec![0.0; edge.len().saturating_sub(1)];
    for &(a, b) in pieces {
        for i in 0..totals.len() {
            let (lo, hi) = (edge[i], edge[i + 1]);
            if hi <= a {
                continue;
            }
            if lo >= b {
                break;
            }
            totals[i] += b.min(hi) - a.max(lo);
        }
    }
    totals
}

/// "45s", "25m", "1h 02m". Rounds half to even, as Python's round() does.
pub fn fmt(secs: f64) -> String {
    let secs = secs.round_ties_even() as i64;
    if secs < 60 {
        return format!("{secs}s");
    }
    let (h, m) = (secs / 60 / 60, secs / 60 % 60);
    if h > 0 {
        format!("{h}h {m:02}m")
    } else {
        format!("{m}m")
    }
}

// ── queries ───────────────────────────────────────────────────────────────────

/// A segment overlapping a window, clipped to it (`a`..`b`), with its session's fields.
#[derive(Debug, Clone)]
pub struct Seg {
    pub id: i64,
    pub kind: String,
    pub label: String,
    pub planned: Option<f64>,
    pub started: f64,
    pub status: String,
    pub a: f64,
    pub b: f64,
}

pub fn load(conn: &Connection, lo: f64, hi: f64) -> rusqlite::Result<Vec<Seg>> {
    let mut stmt = conn.prepare(
        "SELECT g.start, g.secs, s.id, s.kind, s.label, s.planned, s.started, s.status, s.active \
         FROM segments g JOIN sessions s ON s.id = g.session_id \
         WHERE g.start < ?1 AND g.start + g.secs > ?2 AND s.active >= ?3 ORDER BY g.start",
    )?;
    let rows = stmt.query_map(params![hi, lo, MIN_SECS], |r| {
        let start: f64 = r.get(0)?;
        let secs: f64 = r.get(1)?;
        Ok(Seg {
            id: r.get(2)?,
            kind: r.get(3)?,
            label: r.get(4)?,
            planned: r.get(5)?,
            started: r.get(6)?,
            status: r.get(7)?,
            a: start.max(lo),
            b: (start + secs).min(hi),
        })
    })?;
    rows.collect()
}

/// Seconds tracked on every day in [first, last].
pub fn day_totals<Tz: TimeZone>(conn: &Connection, tz: &Tz, first: NaiveDate, last: NaiveDate) -> rusqlite::Result<Vec<(NaiveDate, f64)>> {
    let end = last + Duration::days(1);
    let segs = load(conn, ts(tz, first), ts(tz, end))?;
    let (edge, _) = edges(tz, first, end, Unit::Day);
    let pieces: Vec<_> = segs.iter().map(|s| (s.a, s.b)).collect();
    Ok(spread(&pieces, &edge)
        .into_iter()
        .enumerate()
        .map(|(i, v)| (first + Duration::days(i as i64), v))
        .collect())
}

/// Days in a row with at least a minute tracked, counting today only once it has started.
pub fn streak<Tz: TimeZone>(conn: &Connection, tz: &Tz, today: NaiveDate) -> rusqlite::Result<u32> {
    let days: std::collections::HashMap<NaiveDate, f64> = day_totals(conn, tz, today - Duration::days(400), today)?.into_iter().collect();
    let has = |d: &NaiveDate| days.get(d).copied().unwrap_or(0.0) >= STREAK_SECS;
    let mut d = if has(&today) { today } else { today - Duration::days(1) };
    let mut n = 0;
    while has(&d) {
        n += 1;
        d -= Duration::days(1);
    }
    Ok(n)
}

#[derive(Debug, Clone, Serialize)]
pub struct Bucket {
    pub name: String,
    pub secs: f64,
    pub start: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LabelTotal {
    pub label: String,
    pub secs: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionRow {
    pub label: String,
    pub kind: String,
    pub status: String,
    pub started: f64,
    pub secs: f64,
    pub planned: Option<f64>,
}

/// The summary of one period, field for field the original's dict.
#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub period: String,
    pub title: String,
    pub unit: Unit,
    pub label: Option<String>,
    pub range: [String; 2],
    pub total: f64,
    pub count: usize,
    pub avg: f64,
    pub longest: f64,
    pub completion: Option<f64>,
    pub buckets: Vec<Bucket>,
    pub labels: Vec<LabelTotal>,
    pub hours: Vec<f64>,
    pub sessions: Vec<SessionRow>,
}

pub fn summarize<Tz: TimeZone>(
    conn: &Connection,
    tz: &Tz,
    period: &str,
    label: Option<&str>,
    today: NaiveDate,
) -> rusqlite::Result<Summary> {
    let (start, end, unit) = bounds(period, today).ok_or_else(|| rusqlite::Error::InvalidParameterName(period.into()))?;
    let (lo, hi) = (ts(tz, start), ts(tz, end));
    let segs: Vec<Seg> = load(conn, lo, hi)?
        .into_iter()
        .filter(|s| label.is_none_or(|l| s.label == l))
        .collect();
    let (edge, names) = edges(tz, start, end, unit);
    let pieces: Vec<(f64, f64)> = segs.iter().map(|s| (s.a, s.b)).collect();
    let buckets = spread(&pieces, &edge);

    // insertion-ordered, like Python dicts: ties keep first-seen order through the stable sorts
    let mut by_label: IndexMap<String, f64> = IndexMap::new();
    let mut by_session: IndexMap<i64, f64> = IndexMap::new();
    let mut info: std::collections::HashMap<i64, &Seg> = std::collections::HashMap::new();
    for s in &segs {
        *by_label.entry(s.label.clone()).or_insert(0.0) += s.b - s.a;
        *by_session.entry(s.id).or_insert(0.0) += s.b - s.a;
        info.insert(s.id, s);
    }

    // time of day you actually focus, over the whole period (wall-clock hours, as the original steps them)
    let mut hours = vec![0.0; 24];
    for &(a, b) in &pieces {
        let (naive, mut fold) = from_ts(tz, a);
        let mut h = naive.date().and_hms_opt(naive.hour(), 0, 0).unwrap();
        while local_ts(tz, h, fold) < b {
            let nxt = h + Duration::hours(1);
            hours[h.hour() as usize] += b.min(local_ts(tz, nxt, false)) - a.max(local_ts(tz, h, fold));
            h = nxt;
            fold = false; // datetime arithmetic drops fold
        }
    }

    let timers: Vec<&Seg> = by_session
        .keys()
        .map(|i| info[i])
        .filter(|s| s.kind == "timer" && s.status != "open")
        .collect();
    let done = timers.iter().filter(|s| s.status == "completed").count();
    let total: f64 = by_label.values().sum();
    let mut sessions: Vec<SessionRow> = by_session
        .iter()
        .map(|(i, &secs)| {
            let s = info[i];
            SessionRow {
                label: s.label.clone(),
                kind: s.kind.clone(),
                status: s.status.clone(),
                started: s.started,
                secs,
                planned: s.planned,
            }
        })
        .collect();
    sessions.sort_by(|x, y| y.started.total_cmp(&x.started));
    sessions.truncate(50);
    let mut labels: Vec<LabelTotal> = by_label.iter().map(|(k, &v)| LabelTotal { label: k.clone(), secs: v }).collect();
    labels.sort_by(|x, y| y.secs.total_cmp(&x.secs));

    Ok(Summary {
        period: period.into(),
        title: title(period).into(),
        unit,
        label: label.map(Into::into),
        range: [start.to_string(), (end - Duration::days(1)).to_string()],
        total,
        count: by_session.len(),
        avg: if by_session.is_empty() {
            0.0
        } else {
            total / by_session.len() as f64
        },
        longest: by_session.values().copied().fold(0.0, f64::max),
        completion: if timers.is_empty() {
            None
        } else {
            Some(done as f64 / timers.len() as f64)
        },
        buckets: names
            .into_iter()
            .zip(buckets)
            .enumerate()
            .map(|(i, (name, secs))| Bucket {
                name,
                secs,
                start: edge[i],
            })
            .collect(),
        labels,
        hours,
        sessions,
    })
}

/// Labels the user has used, longest first (for matching "how long on X").
pub fn known_labels(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT label FROM sessions WHERE label != ?1 GROUP BY label ORDER BY length(label) DESC")?;
    let rows = stmt.query_map(params![UNTITLED], |r| r.get(0))?;
    rows.collect()
}

/// The line the island says for a summary.
pub fn one_liner(s: &Summary) -> String {
    let plural = if s.count != 1 { "s" } else { "" };
    if s.count == 0 {
        let what = s.label.as_ref().map(|l| format!(" on {l}")).unwrap_or_default();
        return format!(
            "Nothing tracked{what} {} yet. Try “timer 25 min for study”.",
            s.title.to_lowercase()
        );
    }
    let head = format!("{}: {}", s.title, fmt(s.total));
    if let Some(l) = &s.label {
        return format!("{head} on {l} over {} session{plural}.", s.count);
    }
    let top = &s.labels[0];
    let tail = if s.labels.len() > 1 || top.label != UNTITLED {
        format!(" · most on {} ({})", top.label, fmt(top.secs))
    } else {
        String::new()
    };
    format!("{head} over {} session{plural}{tail}.", s.count)
}
