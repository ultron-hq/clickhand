//! Text → skill → reply, ported from ultron/brain.py (routing) and skills/__init__.py.
//!
//! The skill registry is static (`skills.json`, extracted from the original in its
//! order) instead of an importlib scan. Skills and the model planner plug in through
//! traits, so wave 2 adds real skills without touching the routing. Per ADR-002 there
//! is no Laya: when no rule fires, the model planner is asked, as the original does
//! whenever Laya is unsure or unavailable.
//!
//! The desktop/workflow follow-up branches arrive with the desktop loop (wave 4);
//! with no desktop attached they never fire in the original either.
use std::sync::LazyLock;

use regex::Regex;
use serde::Deserialize;

use crate::memory::Memory;

pub const MIN_CONFIDENCE: f64 = 0.4;
pub const ESCALATE_BELOW: f64 = 0.75;
/// "no, I meant spotify" within this long after a request fixes (and teaches) that request.
pub const CORRECTION_WINDOW: f64 = 180.0;

static CORRECTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(no|nope|nah|wrong|not that)\b[\s,.!-]*(i\s+(meant|mean|said|wanted)\s+|it'?s\s+|use\s+|try\s+)?(?P<x>.+)$")
        .unwrap()
});
static QUESTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*(who|what|what's|whats|when|where|why|how|which|is|are|do|does|did|can)\b|\?\s*$").unwrap());

/// What the user meant, when this is a correction like "no, I meant spotify".
pub fn correction(text: &str) -> Option<String> {
    // Python's `$` also matches before one trailing newline
    let t = text.strip_suffix('\n').unwrap_or(text);
    CORRECTION.captures(t).map(|c| c["x"].to_string())
}

pub fn is_question(text: &str) -> bool {
    QUESTION.is_match(text) || text.strip_suffix('\n').is_some_and(|t| QUESTION.is_match(t))
}

#[derive(Debug, Deserialize)]
struct SkillRecord {
    name: String,
    only_match: bool,
    pattern: Option<String>,
    ignore_case: bool,
    description: String,
    usage: Option<String>,
}

/// One skill as the router sees it.
pub struct SkillInfo {
    pub name: String,
    pub only_match: bool,
    pub description: String,
    pub usage: Option<String>,
    matcher: Option<fancy_regex::Regex>,
}

impl SkillInfo {
    pub fn matches(&self, text: &str) -> bool {
        self.matcher.as_ref().is_some_and(|m| m.is_match(text).unwrap_or(false))
    }
}

/// The skills, in the original's order ("none", the catch-all, last).
pub struct Registry {
    pub skills: Vec<SkillInfo>,
}

impl Registry {
    pub fn builtin() -> Registry {
        let records: Vec<SkillRecord> = serde_json::from_str(include_str!("skills.json")).expect("skills.json");
        let skills = records
            .into_iter()
            .map(|r| SkillInfo {
                matcher: r.pattern.map(|p| {
                    let p = if r.ignore_case { format!("(?i){p}") } else { p };
                    fancy_regex::Regex::new(&p).unwrap_or_else(|e| panic!("skill {} pattern: {e}", r.name))
                }),
                name: r.name,
                only_match: r.only_match,
                description: r.description,
                usage: r.usage,
            })
            .collect();
        Registry { skills }
    }

    pub fn get(&self, name: &str) -> Option<&SkillInfo> {
        self.skills.iter().find(|s| s.name == name)
    }

    /// The skill whose rule fires. Keyword-only skills first, so "start a stopwatch"
    /// isn't taken as "start <app>".
    pub fn rule(&self, text: &str) -> Option<&str> {
        for first in ["tasks", "workflow", "desktop"] {
            if let Some(s) = self.get(first).filter(|s| s.matches(text)) {
                return Some(s.name.as_str());
            }
        }
        let only = self.skills.iter().filter(|s| s.only_match);
        let rest = self.skills.iter().filter(|s| !s.only_match);
        only.chain(rest).find(|s| s.matches(text)).map(|s| s.name.as_str())
    }
}

/// Who chose the action (gates destructive actions: only the user's own requests may delete).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    User,
    Laya,
    Learned,
    Haiku,
}

impl Origin {
    pub fn as_str(self) -> &'static str {
        match self {
            Origin::User => "user",
            Origin::Laya => "laya",
            Origin::Learned => "learned",
            Origin::Haiku => "haiku",
        }
    }
}

/// What a skill may know about the request.
pub struct Ctx {
    pub origin: Origin,
    pub request: String,
    pub dry_run: bool,
}

impl Default for Ctx {
    fn default() -> Self {
        Ctx {
            origin: Origin::User,
            request: String::new(),
            dry_run: false,
        }
    }
}

/// A skill couldn't make sense of the text, or it failed.
#[derive(Debug, Clone)]
pub enum SkillError {
    /// The router may ask the planner; this message is shown only if that fails too.
    Unclear(String),
    /// A broken skill must not take Ultron down: reported as "That failed: …".
    Failed(String),
}

/// Runs skills by name. Real skills arrive in wave 2.
pub trait Skills {
    fn run(&mut self, name: &str, text: &str, ctx: &mut Ctx) -> Result<String, SkillError>;
}

/// The model's answer for a request it was asked about.
#[derive(Debug, Clone, PartialEq)]
pub enum Plan {
    Answer(String),
    Run { skill: String, command: String },
}

pub trait Planner {
    fn plan(&mut self, text: &str, notes: &[String]) -> Option<Plan>;
}

/// How a request was resolved. `command` is the text the skill actually ran.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub reply: String,
    pub skill: String,
    pub conf: f64,
    pub via: String,
    pub command: Option<String>,
    pub ok: bool,
}

fn outcome(reply: impl Into<String>, skill: &str, conf: f64, via: &str, command: Option<String>, ok: bool) -> Outcome {
    Outcome {
        reply: reply.into(),
        skill: skill.into(),
        conf,
        via: via.into(),
        command,
        ok,
    }
}

const NO_LEARN: [&str; 4] = ["none", "desktop", "workflow", "screen"];

pub struct Router<C: Fn() -> f64> {
    pub registry: Registry,
    pub memory: Option<Memory<C>>,
    pub planner: Option<Box<dyn Planner>>,
    /// (text, monotonic time) of the last real request, for corrections.
    pub last: Option<(String, f64)>,
}

impl<C: Fn() -> f64> Router<C> {
    pub fn new(memory: Option<Memory<C>>, planner: Option<Box<dyn Planner>>) -> Self {
        Router {
            registry: Registry::builtin(),
            memory,
            planner,
            last: None,
        }
    }

    fn run_skill(&self, skills: &mut dyn Skills, name: &str, text: &str, ctx: &mut Ctx) -> Result<String, String> {
        match skills.run(name, text, ctx) {
            Ok(reply) => Ok(reply),
            Err(SkillError::Unclear(msg)) => Err(msg),
            Err(SkillError::Failed(msg)) => Ok(format!("That failed: {msg}")),
        }
    }

    /// Rules first; with no local router (ADR-002) everything else is "none".
    pub fn decide(&self, text: &str) -> (String, f64, bool) {
        match self.registry.rule(text) {
            Some(name) => (name.to_string(), 1.0, true),
            None => ("none".to_string(), 0.0, false),
        }
    }

    pub fn route(&mut self, text: &str, skills: &mut dyn Skills, ctx: &mut Ctx) -> Outcome {
        if ["new conversation", "clear conversation", "forget this conversation"].contains(&text.trim().to_lowercase().as_str()) {
            return outcome(
                "Conversation cleared. What would you like to explore?",
                "screen",
                1.0,
                "rule",
                None,
                true,
            );
        }
        // your phrases: "my notes" -> "obsidian"
        let text = match &self.memory {
            Some(m) => m.expand(text).unwrap_or_else(|_| text.to_string()),
            None => text.to_string(),
        };
        let ruled = self.registry.rule(&text).is_some();

        if !ruled {
            if let Some(Ok(Some(hit))) = self.memory.as_ref().map(|m| m.lookup(&text)) {
                if let Some(answer) = hit.answer {
                    if let Some(m) = &self.memory {
                        let _ = m.used(hit.id);
                    }
                    return outcome(answer, "llm", 1.0, "learned", None, true);
                }
                ctx.origin = Origin::Learned;
                let command = hit.command.clone().unwrap_or_default();
                let ran = self.run_skill(skills, &hit.skill, &command, ctx);
                if let Some(m) = &self.memory {
                    match ran {
                        Ok(reply) => {
                            let _ = m.used(hit.id);
                            return outcome(reply, &hit.skill, 1.0, "learned", Some(command), true);
                        }
                        Err(_) => {
                            let _ = m.failed(hit.id); // stale (app removed, bad fill): relearn
                        }
                    }
                }
            }
        }

        if !ruled && is_question(&text) {
            if let Some(Ok(Some(fact))) = self.memory.as_ref().map(|m| m.recall(&text)) {
                return outcome(format!("You told me: {fact}"), "memory", 1.0, "memory", None, true);
            }
        }

        let (skill, conf, by_rule) = self.decide(&text);
        // nothing clear: ask the model before doing anything
        let unsure = !by_rule && (skill == "none" || skill == "web_search" || conf < ESCALATE_BELOW);
        if unsure {
            if let Some(planned) = self.ask_planner(&text, skills, ctx) {
                return planned;
            }
            if skill != "none" && conf < MIN_CONFIDENCE {
                return outcome(
                    format!("Not sure what you mean — {}? Try rephrasing.", skill.replace('_', " ")),
                    &skill,
                    conf,
                    "laya",
                    None,
                    false,
                );
            }
        }
        let via = if by_rule { "rule" } else { "laya" };
        ctx.origin = if by_rule { Origin::User } else { Origin::Laya };
        match self.run_skill(skills, &skill, &text, ctx) {
            Ok(reply) => {
                if via == "laya" && !NO_LEARN.contains(&skill.as_str()) {
                    if let Some(mem) = &mut self.memory {
                        let _ = mem.learn(&text, &skill, Some(&text), None, "laya");
                    }
                }
                outcome(reply, &skill, conf, via, Some(text), true)
            }
            Err(unclear) => {
                let planned = if unsure { None } else { self.ask_planner(&text, skills, ctx) };
                planned.unwrap_or_else(|| outcome(unclear, &skill, conf, via, None, false))
            }
        }
    }

    /// Ask the model. Successful plans are learned, so the same request (or one fitting
    /// its pattern) skips the model next time.
    fn ask_planner(&mut self, text: &str, skills: &mut dyn Skills, ctx: &mut Ctx) -> Option<Outcome> {
        self.planner.as_ref()?;
        let notes: Vec<String> = match &self.memory {
            Some(m) => m
                .facts_about(text, 5)
                .map(|v| v.into_iter().map(|(_, t)| t).collect())
                .unwrap_or_default(),
            None => Vec::new(),
        };
        let plan = self.planner.as_mut()?.plan(text, &notes)?;
        match plan {
            Plan::Answer(answer) => {
                if notes.is_empty() {
                    // answers built on your facts can change; don't freeze them
                    if let Some(mem) = &mut self.memory {
                        let _ = mem.learn(text, "llm", None, Some(&answer), "haiku");
                    }
                }
                Some(outcome(answer, "llm", 1.0, "haiku", None, true))
            }
            Plan::Run { skill, command } => {
                ctx.origin = Origin::Haiku;
                match self.run_skill(skills, &skill, &command, ctx) {
                    Err(unclear) => Some(outcome(unclear, &skill, 1.0, "haiku", Some(command), false)),
                    Ok(reply) => {
                        if !["memory", "time_report", "desktop", "workflow", "screen"].contains(&skill.as_str()) {
                            if let Some(mem) = &mut self.memory {
                                let _ = mem.learn(text, &skill, Some(&command), None, "haiku");
                            }
                        }
                        Some(outcome(reply, &skill, 1.0, "haiku", Some(command), true))
                    }
                }
            }
        }
    }

    /// "no, I meant spotify": redo the last request as that, and remember the fix.
    pub fn correct(&mut self, text: &str, now: f64, skills: &mut dyn Skills, ctx: &mut Ctx) -> Option<Outcome> {
        let meant = correction(text)?;
        let (wrong, at) = self.last.clone()?;
        if now - at > CORRECTION_WINDOW {
            return None;
        }
        let mut res = self.route(&meant, skills, ctx);
        if res.ok && !NO_LEARN.contains(&res.skill.as_str()) {
            if let Some(mem) = &mut self.memory {
                if res.skill == "llm" {
                    let _ = mem.learn(&wrong, "llm", None, Some(&res.reply), "correction");
                } else if let Some(cmd) = &res.command {
                    let _ = mem.learn(&wrong, &res.skill, Some(cmd), None, "correction");
                }
                res.reply = format!("{} Got it, I'll do that next time.", res.reply.trim_end());
            }
        }
        self.last = None;
        res.via = "correction".into();
        Some(res)
    }
}
