//! Ultron's memory: facts you tell it, your own phrases, and what it learned from the
//! model, ported from ultron/memory.py.
//!
//! Learning is distillation, not weight training: every request the model resolves
//! (and every "no, I meant …") is kept as an exact phrase and, when the request and
//! the plan share words, as a template — "show me some cat videos" → "search youtube
//! for cat videos" becomes `^show\s+me\s+some\s+(.+?)$` → "search youtube for {0}".
//!
//! Templates are stored as regex strings in the shared SQLite file, so this module
//! writes exactly the strings Python's `re.escape` would (ADR-005: both versions read
//! the same data). Only what you type is stored, and it stays on your machine.
use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::sessions;

pub const ANSWER_TTL: f64 = 30.0 * 86400.0; // cached model answers are re-asked after a month
pub const MAX_SLOT_WORDS: usize = 6;

static SECRET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(passwords?|passwd|passcodes?|pin( code)?|secrets?|api[\s_-]?keys?|tokens?|private keys?|seed phrase|recovery phrase|credit card|card number|cvv|ssn|social security)\b",
    )
    .unwrap()
});
static ADDRESS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*((hey|ok|okay|yo)\s+)?ultron\b[\s,:!-]*|^\s*(please|pls)\s+").unwrap());
static KEEP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^\w%'.:/+#@-]+").unwrap());
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
static WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[a-z0-9%.@:/-]+").unwrap());
static PERSONAL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(my|i|me|mine|i'm|im)\b").unwrap());
static LAST: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(that|it|this|the last (one|thing))$").unwrap());

const STOP: &str = "a an the and or but of to in on at for with by from up about into over after is are
was were be been being am do does did have has had i me my mine you your it its this that these
those some any please can could would should will just so very what whats what's who whom whose
when where why how which there here then than too also not no yes hey ultron want wanna gonna
need like let get give show tell";
// Pure function words: a template needs at least one fixed word outside this set.
const FUNCTION: &str = "a an the and or but of to in on at for with by from up about into is are was
were be am do does did i me my you your it its this that these those some any please can could
would will just so";

static STOP_SET: LazyLock<HashSet<&'static str>> = LazyLock::new(|| STOP.split_whitespace().collect());
static FUNCTION_SET: LazyLock<HashSet<&'static str>> = LazyLock::new(|| FUNCTION.split_whitespace().collect());

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS facts (
    id INTEGER PRIMARY KEY, text TEXT NOT NULL, norm TEXT NOT NULL UNIQUE, created REAL NOT NULL);
CREATE TABLE IF NOT EXISTS aliases (
    id INTEGER PRIMARY KEY, phrase TEXT NOT NULL UNIQUE, meaning TEXT NOT NULL, created REAL NOT NULL);
CREATE TABLE IF NOT EXISTS learned (
    id       INTEGER PRIMARY KEY,
    kind     TEXT NOT NULL,              -- exact | template
    key      TEXT NOT NULL UNIQUE,       -- normalized request, or the template regex
    skill    TEXT NOT NULL,              -- a skill name, or 'llm' for a cached answer
    command  TEXT,                       -- plan (template: with {0} {1} slots)
    answer   TEXT,
    source   TEXT NOT NULL,              -- haiku | correction
    example  TEXT NOT NULL,              -- the request it was learned from
    uses     INTEGER NOT NULL DEFAULT 0,
    fails    INTEGER NOT NULL DEFAULT 0,
    created  REAL NOT NULL,
    last_used REAL);
CREATE TABLE IF NOT EXISTS spots (
    name TEXT PRIMARY KEY, x INTEGER NOT NULL, y INTEGER NOT NULL, created REAL NOT NULL);
CREATE TABLE IF NOT EXISTS requests (
    id INTEGER PRIMARY KEY, at REAL NOT NULL, text TEXT NOT NULL, route TEXT NOT NULL, ms REAL);
";

/// Python's `str.strip(chars)`.
fn strip<'a>(s: &'a str, chars: &str) -> &'a str {
    s.trim_matches(|c| chars.contains(c))
}

/// Python's `re.escape` (3.7+): only these characters get a backslash.
pub fn py_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if "()[]{}?*+-|^$\\.&~# \t\n\r\x0b\x0c".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Python's `str.format` for `{0}`-style templates; None where Python would raise.
fn py_format(template: &str, args: &[&str]) -> Option<String> {
    let mut out = String::new();
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                out.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                out.push('}');
            }
            '{' => {
                let mut num = String::new();
                for d in chars.by_ref() {
                    if d == '}' {
                        break;
                    }
                    num.push(d);
                }
                out.push_str(args.get(num.parse::<usize>().ok()?)?);
            }
            '}' => return None,
            c => out.push(c),
        }
    }
    Some(out)
}

pub fn normalize(text: &str) -> String {
    let mut t = text.to_lowercase().replace('’', "'").replace(['“', '”'], "\"");
    loop {
        // "hey ultron, please …" → "…"
        let next = ADDRESS.replace(&t, "").into_owned();
        if next == t {
            break;
        }
        t = next;
    }
    let t = KEEP.replace_all(&t, " ");
    let t = SPACES.replace_all(&t, " ");
    strip(t.trim(), " .!?,'").to_string()
}

/// Content words, lightly stemmed, for matching facts to questions.
pub fn words(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let mut out = Vec::new();
    for m in WORD.find_iter(&lower) {
        let mut w = strip(m.as_str(), ".:-").to_string();
        if w.is_empty() || STOP_SET.contains(w.as_str()) {
            continue;
        }
        for suf in ["ing", "ed", "es", "s"] {
            if w.chars().count() > 4 && w.ends_with(suf) {
                w.truncate(w.len() - suf.len());
                break;
            }
        }
        out.push(w);
    }
    out
}

/// (regex, command template) when the request and the command share a slot.
pub fn build_template(request: &str, command: &str) -> Option<(String, String)> {
    let norm = normalize(request);
    let r: Vec<&str> = norm.split_whitespace().collect();
    let c: Vec<&str> = command.split_whitespace().collect();
    let cl: Vec<String> = c.iter().map(|w| w.to_lowercase()).collect();
    let find = |seq: &[&str]| -> Option<usize> {
        (0..(cl.len() + 1).saturating_sub(seq.len())).find(|&k| cl[k..k + seq.len()].iter().zip(seq).all(|(a, b)| a == b))
    };
    let func = |w: &str| FUNCTION_SET.contains(w);

    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < r.len() {
        if func(r[i]) || find(&r[i..i + 1]).is_none() {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while j < r.len() && find(&r[i..j + 1]).is_some() {
            j += 1;
        }
        while j - 1 > i && func(r[j - 1]) {
            j -= 1; // don't end a slot on "the", "for" …
        }
        spans.push((i, j));
        i = j;
    }
    if spans.is_empty() {
        return None;
    }
    let fixed: Vec<&str> = r
        .iter()
        .enumerate()
        .filter(|(k, _)| !spans.iter().any(|&(a, b)| a <= *k && *k < b))
        .map(|(_, w)| *w)
        .collect();
    if fixed.len() < 2 || fixed.iter().all(|w| func(w)) {
        return None; // too little context to be safe
    }

    let mut parts: Vec<String> = Vec::new();
    let mut tmpl: Vec<Option<String>> = c.iter().map(|w| Some(w.to_string())).collect();
    let mut used: HashSet<usize> = HashSet::new();
    let mut k = 0;
    for (n, &(a, b)) in spans.iter().enumerate() {
        parts.extend(r[k..a].iter().map(|w| py_escape(w)));
        parts.push("(.+?)".into());
        k = b;
        let pos = find(&r[a..b])?;
        if (pos..pos + b - a).any(|p| used.contains(&p)) {
            return None;
        }
        used.extend(pos..pos + b - a);
        tmpl[pos] = Some(format!("{{{n}}}"));
        for slot in tmpl.iter_mut().take(pos + b - a).skip(pos + 1) {
            *slot = None;
        }
    }
    parts.extend(r[k..].iter().map(|w| py_escape(w)));
    let regex = format!("^{}$", parts.join(r"\s+"));
    let template = tmpl.into_iter().flatten().collect::<Vec<_>>().join(" ");
    // sanity: it must reproduce the original
    let re = Regex::new(&regex).ok()?;
    let joined = r.join(" ");
    let caps = re.captures(&joined)?;
    let groups: Vec<&str> = caps.iter().skip(1).map(|g| g.map_or("", |m| m.as_str())).collect();
    if py_format(&template, &groups)?.to_lowercase() != cl.join(" ") {
        return None;
    }
    Some((regex, template))
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Learned {
    pub id: i64,
    pub skill: String,
    pub command: Option<String>,
    pub answer: Option<String>,
    pub kind: String,
}

/// What "forget that" refers to: the last thing stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Change {
    Fact(i64),
    Alias(i64),
    Learned(i64),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Counts {
    pub facts: i64,
    pub aliases: i64,
    pub phrases: i64,
    pub templates: i64,
}

/// Use from one thread (Ultron's worker thread).
pub struct Memory<C: Fn() -> f64> {
    conn: Connection,
    clock: C,
    pub last_change: Option<Change>,
}

impl<C: Fn() -> f64> Memory<C> {
    pub fn open(path: Option<&Path>, clock: C) -> rusqlite::Result<Self> {
        let conn = sessions::connect(path)?;
        conn.execute_batch(SCHEMA)?;
        Ok(Memory {
            conn,
            clock,
            last_change: None,
        })
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    fn texts(&self, sql: &str, args: impl rusqlite::Params) -> rusqlite::Result<Vec<String>> {
        let mut st = self.conn.prepare(sql)?;
        let rows = st.query_map(args, |r| r.get(0))?;
        rows.collect()
    }

    // ── facts ──
    /// (stored?, message).
    pub fn remember(&mut self, text: &str) -> rusqlite::Result<(bool, String)> {
        let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let text = strip(&joined, " .");
        if text.is_empty() {
            return Ok((false, "Remember what?".into()));
        }
        if SECRET.is_match(text) {
            return Ok((
                false,
                "I won't store passwords, keys or card numbers. Use a password manager.".into(),
            ));
        }
        let norm = normalize(text);
        if self
            .conn
            .query_row("SELECT 1 FROM facts WHERE norm=?1", params![norm], |_| Ok(()))
            .optional()?
            .is_some()
        {
            return Ok((false, "I already know that.".into()));
        }
        self.conn.execute(
            "INSERT INTO facts(text, norm, created) VALUES (?1,?2,?3)",
            params![text, norm, (self.clock)()],
        )?;
        self.last_change = Some(Change::Fact(self.conn.last_insert_rowid()));
        Ok((true, "Got it, I'll remember that.".into()))
    }

    /// [(shared content words, fact)] best first.
    pub fn facts_about(&self, query: &str, k: usize) -> rusqlite::Result<Vec<(usize, String)>> {
        let q: HashSet<String> = words(query).into_iter().collect();
        let mut scored = Vec::new();
        for text in self.texts("SELECT text FROM facts ORDER BY created DESC", [])? {
            let shared = words(&text).into_iter().collect::<HashSet<_>>().intersection(&q).count();
            if shared > 0 || q.is_empty() {
                scored.push((shared, text));
            }
        }
        scored.sort_by_key(|a| std::cmp::Reverse(a.0)); // stable, like Python's sort
        scored.truncate(k);
        Ok(scored)
    }

    /// A fact that clearly answers a personal question ("what's my router ip").
    pub fn recall(&self, question: &str) -> rusqlite::Result<Option<String>> {
        let q = words(question);
        if q.is_empty() {
            return Ok(None);
        }
        let personal = PERSONAL.is_match(question);
        let best = self.facts_about(question, 1)?;
        let Some((shared, text)) = best.into_iter().next().filter(|b| b.0 > 0) else {
            return Ok(None);
        };
        let unique = q.iter().collect::<HashSet<_>>().len();
        Ok((shared >= 2 || (personal && shared as f64 / unique as f64 >= 0.5)).then_some(text))
    }

    // ── aliases ──
    pub fn add_alias(&mut self, phrase: &str, meaning: &str) -> rusqlite::Result<(bool, String)> {
        let phrase = strip(&normalize(phrase), "'\" ").to_string();
        let joined = meaning.split_whitespace().collect::<Vec<_>>().join(" ");
        let meaning = strip(&joined, " .'\"").to_string();
        if phrase.is_empty() || meaning.is_empty() {
            return Ok((false, "Say it like: when I say “my notes” I mean obsidian.".into()));
        }
        if phrase == normalize(&meaning) {
            return Ok((false, "That's the same thing.".into()));
        }
        self.conn.execute(
            "INSERT INTO aliases(phrase, meaning, created) VALUES (?1,?2,?3) ON CONFLICT(phrase) DO UPDATE SET meaning=excluded.meaning",
            params![phrase, meaning, (self.clock)()],
        )?;
        let id: i64 = self
            .conn
            .query_row("SELECT id FROM aliases WHERE phrase=?1", params![phrase], |r| r.get(0))?;
        self.last_change = Some(Change::Alias(id));
        Ok((true, format!("Got it: “{phrase}” means {meaning}.")))
    }

    /// Replace your phrases with what they mean ("open my notes" → "open obsidian").
    pub fn expand(&self, text: &str) -> rusqlite::Result<String> {
        let mut st = self
            .conn
            .prepare("SELECT phrase, meaning FROM aliases ORDER BY length(phrase) DESC")?;
        let rows: Vec<(String, String)> = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        let mut out = text.to_string();
        for (phrase, meaning) in rows {
            let words: Vec<String> = phrase.split_whitespace().map(py_escape).collect();
            // lookaround: fancy-regex, since the regex crate has none
            let pat = format!(r"(?i)(?<![\w']){}(?![\w'])", words.join(r"\s+"));
            if let Ok(re) = fancy_regex::Regex::new(&pat) {
                out = re.replace_all(&out, fancy_regex::NoExpand(&meaning)).into_owned();
            }
        }
        Ok(out)
    }

    // ── learned ──
    /// Keep a resolved request. Corrections override; the model never overrides a correction.
    pub fn learn(&mut self, request: &str, skill: &str, command: Option<&str>, answer: Option<&str>, source: &str) -> rusqlite::Result<()> {
        let norm = normalize(request);
        if norm.is_empty() {
            return Ok(());
        }
        let now = (self.clock)();
        let mut rows: Vec<(&str, String, Option<String>)> = vec![("exact", norm, command.map(Into::into))];
        if let Some(cmd) = command.filter(|_| skill != "llm") {
            if let Some((key, tmpl)) = build_template(request, cmd) {
                rows.push(("template", key, Some(tmpl)));
            }
        }
        for (kind, key, cmd) in rows {
            let old: Option<(i64, String)> = self
                .conn
                .query_row("SELECT id, source FROM learned WHERE key=?1", params![key], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .optional()?;
            if old.as_ref().is_some_and(|o| o.1 == "correction") && source != "correction" {
                continue;
            }
            let lid = if let Some((id, _)) = old {
                self.conn.execute(
                    "UPDATE learned SET skill=?1, command=?2, answer=?3, source=?4, example=?5, created=?6, fails=0 WHERE id=?7",
                    params![skill, cmd, answer, source, request, now, id],
                )?;
                id
            } else {
                self.conn.execute(
                    "INSERT INTO learned(kind, key, skill, command, answer, source, example, created) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                    params![kind, key, skill, cmd, answer, source, request, now],
                )?;
                self.conn.last_insert_rowid()
            };
            if kind == "exact" {
                self.last_change = Some(Change::Learned(lid));
            }
        }
        Ok(())
    }

    /// A learned phrase or template for this request.
    pub fn lookup(&self, text: &str) -> rusqlite::Result<Option<Learned>> {
        let norm = normalize(text);
        if norm.is_empty() {
            return Ok(None);
        }
        type Row = (i64, String, Option<String>, Option<String>, f64);
        let exact: Option<Row> = self
            .conn
            .query_row(
                "SELECT id, skill, command, answer, created FROM learned WHERE kind='exact' AND key=?1",
                params![norm],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .optional()?;
        if let Some((id, skill, command, answer, created)) = exact {
            if answer.is_some() && (self.clock)() - created > ANSWER_TTL {
                self.conn.execute("DELETE FROM learned WHERE id=?1", params![id])?;
            } else {
                return Ok(Some(Learned {
                    id,
                    skill,
                    command,
                    answer,
                    kind: "exact".into(),
                }));
            }
        }
        // most specific template first (longest pattern = most fixed words)
        let mut st = self
            .conn
            .prepare("SELECT id, skill, command, key FROM learned WHERE kind='template' ORDER BY length(key) DESC")?;
        let rows: Vec<(i64, String, Option<String>, String)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (id, skill, command, key) in rows {
            let Ok(re) = Regex::new(&key) else { continue };
            let Some(caps) = re.captures(&norm) else { continue };
            let groups: Vec<&str> = caps.iter().skip(1).map(|g| g.map_or("", |m| m.as_str())).collect();
            if groups.iter().any(|g| g.split_whitespace().count() > MAX_SLOT_WORDS) {
                continue;
            }
            let Some(command) = command.as_deref().and_then(|c| py_format(c, &groups)) else {
                continue;
            };
            return Ok(Some(Learned {
                id,
                skill,
                command: Some(command),
                answer: None,
                kind: "template".into(),
            }));
        }
        Ok(None)
    }

    pub fn used(&self, lid: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE learned SET uses=uses+1, last_used=?1 WHERE id=?2",
            params![(self.clock)(), lid],
        )?;
        Ok(())
    }

    /// The plan didn't work. Drop it once it fails more than it works.
    pub fn failed(&self, lid: i64) -> rusqlite::Result<()> {
        self.conn.execute("UPDATE learned SET fails=fails+1 WHERE id=?1", params![lid])?;
        self.conn.execute(
            "DELETE FROM learned WHERE id=?1 AND (kind='exact' OR (fails >= 3 AND fails > uses))",
            params![lid],
        )?;
        Ok(())
    }

    // ── forget ──
    /// Human descriptions of what was removed.
    pub fn forget(&mut self, what: &str) -> rusqlite::Result<Vec<String>> {
        let what = strip(what, " .!?\"'“”");
        if LAST.is_match(what) {
            let Some(change) = self.last_change.take() else { return Ok(vec![]) };
            return match change {
                Change::Fact(id) => {
                    let text: Option<String> = self
                        .conn
                        .query_row("SELECT text FROM facts WHERE id=?1", params![id], |r| r.get(0))
                        .optional()?;
                    let Some(text) = text else { return Ok(vec![]) };
                    self.conn.execute("DELETE FROM facts WHERE id=?1", params![id])?;
                    Ok(vec![format!("“{text}”")])
                }
                Change::Alias(id) => {
                    let row: Option<(String, String)> = self
                        .conn
                        .query_row("SELECT phrase, meaning FROM aliases WHERE id=?1", params![id], |r| {
                            Ok((r.get(0)?, r.get(1)?))
                        })
                        .optional()?;
                    let Some((phrase, meaning)) = row else { return Ok(vec![]) };
                    self.conn.execute("DELETE FROM aliases WHERE id=?1", params![id])?;
                    Ok(vec![format!("“{phrase}” means {meaning}")])
                }
                Change::Learned(id) => {
                    let example: Option<String> = self
                        .conn
                        .query_row("SELECT example FROM learned WHERE id=?1", params![id], |r| r.get(0))
                        .optional()?;
                    let Some(example) = example else { return Ok(vec![]) };
                    self.conn.execute("DELETE FROM learned WHERE example=?1", params![example])?;
                    Ok(vec![format!("what “{example}” means")])
                }
            };
        }
        let q: HashSet<String> = words(what).into_iter().collect();
        let norm = normalize(what);
        let mut gone = Vec::new();
        if q.is_empty() && norm.is_empty() {
            return Ok(gone);
        }
        let covers = |text: &str| !q.is_empty() && q.is_subset(&words(text).into_iter().collect());
        let facts: Vec<(i64, String, String)> = {
            let mut st = self.conn.prepare("SELECT id, text, norm FROM facts")?;
            let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        for (id, text, fnorm) in facts {
            if covers(&text) || norm == fnorm {
                self.conn.execute("DELETE FROM facts WHERE id=?1", params![id])?;
                gone.push(format!("“{text}”"));
            }
        }
        let aliases: Vec<(i64, String, String)> = {
            let mut st = self.conn.prepare("SELECT id, phrase, meaning FROM aliases")?;
            let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        for (id, phrase, meaning) in aliases {
            if norm == phrase || norm == normalize(&meaning) || covers(&phrase) {
                self.conn.execute("DELETE FROM aliases WHERE id=?1", params![id])?;
                gone.push(format!("“{phrase}” means {meaning}"));
            }
        }
        let learned: Vec<(String, String)> = {
            let mut st = self.conn.prepare("SELECT key, example FROM learned WHERE kind='exact'")?;
            let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        for (key, example) in learned {
            if norm == key {
                self.conn.execute("DELETE FROM learned WHERE example=?1", params![example])?;
                gone.push(format!("what “{example}” means"));
            }
        }
        Ok(gone)
    }

    pub fn forget_everything(&mut self) -> rusqlite::Result<Counts> {
        let n = self.counts()?;
        for table in ["facts", "aliases", "learned"] {
            self.conn.execute(&format!("DELETE FROM {table}"), [])?;
        }
        self.last_change = None;
        Ok(n)
    }

    // ── stats ──
    pub fn counts(&self) -> rusqlite::Result<Counts> {
        let one = |sql: &str| self.conn.query_row(sql, [], |r| r.get::<_, i64>(0));
        Ok(Counts {
            facts: one("SELECT count(*) FROM facts")?,
            aliases: one("SELECT count(*) FROM aliases")?,
            phrases: one("SELECT count(*) FROM learned WHERE kind='exact'")?,
            templates: one("SELECT count(*) FROM learned WHERE kind='template'")?,
        })
    }

    pub fn log_request(&self, text: &str, route: &str, ms: Option<f64>) {
        let text: String = text.chars().take(300).collect();
        let _ = self.conn.execute(
            "INSERT INTO requests(at, text, route, ms) VALUES (?1,?2,?3,?4)",
            params![(self.clock)(), text, route, ms],
        );
    }

    pub fn routes(&self, days: f64) -> rusqlite::Result<BTreeMap<String, i64>> {
        let since = (self.clock)() - days * 86400.0;
        let mut st = self
            .conn
            .prepare("SELECT route, count(*) n FROM requests WHERE at >= ?1 GROUP BY route")?;
        let rows = st.query_map(params![since], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect()
    }

    // ── spots: remembered screen positions ──
    pub fn set_spot(&self, name: &str, x: f64, y: f64) -> rusqlite::Result<String> {
        let name = strip(&normalize(name), "'\" ").to_string();
        self.conn.execute(
            "INSERT INTO spots(name, x, y, created) VALUES (?1,?2,?3,?4) ON CONFLICT(name) DO UPDATE SET x=excluded.x, y=excluded.y, created=excluded.created",
            params![name, x.trunc() as i64, y.trunc() as i64, (self.clock)()],
        )?;
        Ok(name)
    }

    pub fn get_spot(&self, name: &str) -> rusqlite::Result<Option<(i64, i64)>> {
        let name = strip(&normalize(name), "'\" ").to_string();
        self.conn
            .query_row("SELECT x, y FROM spots WHERE name=?1", params![name], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()
    }

    pub fn spots(&self) -> rusqlite::Result<Vec<(String, i64, i64)>> {
        let mut st = self.conn.prepare("SELECT name, x, y FROM spots ORDER BY name")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        rows.collect()
    }

    pub fn forget_spot(&self, name: &str) -> rusqlite::Result<bool> {
        let name = strip(&normalize(name), "'\" ").to_string();
        Ok(self.conn.execute("DELETE FROM spots WHERE name=?1", params![name])? > 0)
    }

    pub fn facts(&self) -> rusqlite::Result<Vec<String>> {
        self.texts("SELECT text FROM facts ORDER BY created", [])
    }

    pub fn aliases(&self) -> rusqlite::Result<Vec<(String, String)>> {
        let mut st = self.conn.prepare("SELECT phrase, meaning FROM aliases ORDER BY phrase")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect()
    }
}
