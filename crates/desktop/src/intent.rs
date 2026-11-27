//! The requests the app answers by itself, without a model: timers and the stopwatch.
use std::sync::LazyLock;

use regex::Regex;

#[derive(Debug, PartialEq)]
pub enum Intent {
    Timer {
        seconds: f64,
        label: Option<String>,
    },
    Stopwatch {
        label: Option<String>,
    },
    Pause,
    Resume,
    Stop,
    /// Anything else: hand it to the agent.
    Agent,
}

static TIMER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)^(?:please\s+)?(?:start|set|begin|run)?\s*(?:a|an|the)?\s*(?:timer\s+(?:for\s+)?)?
        (\d+(?:\.\d+)?)\s*(h|hr|hrs|hours?|m|min|mins|minutes?|s|sec|secs|seconds?)\b
        (?:\s+timer)?(?:\s+(?:for|on|to)\s+(.+?))?\s*$",
    )
    .unwrap()
});
static STOPWATCH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(?:start|begin|run)?\s*(?:a|the)?\s*stopwatch(?:\s+(?:for|on)\s+(.+?))?\s*$").unwrap());

pub fn parse(text: &str) -> Intent {
    let t = text.trim().trim_end_matches(['.', '!']);
    let low = t.to_lowercase();
    match low.as_str() {
        "pause" | "pause timer" | "pause the timer" | "hold" => return Intent::Pause,
        "resume" | "continue" | "resume timer" | "resume the timer" | "unpause" => return Intent::Resume,
        "stop" | "stop timer" | "stop the timer" | "cancel" | "cancel timer" | "cancel the timer" | "stop stopwatch"
        | "stop the stopwatch" | "done" => return Intent::Stop,
        _ => {}
    }
    if let Some(c) = TIMER.captures(t) {
        let n: f64 = c[1].parse().unwrap_or(0.0);
        let unit = c[2].to_lowercase();
        let seconds = match unit.chars().next() {
            Some('h') => n * 3600.0,
            Some('s') => n,
            _ => n * 60.0,
        };
        if seconds > 0.0 {
            return Intent::Timer {
                seconds,
                label: c.get(3).map(|m| m.as_str().to_string()),
            };
        }
    }
    if let Some(c) = STOPWATCH.captures(t) {
        return Intent::Stopwatch {
            label: c.get(1).map(|m| m.as_str().to_string()),
        };
    }
    Intent::Agent
}

/// 1500 → "25 minutes", 90 → "1 minute 30 seconds", 3600 → "1 hour".
pub fn human(seconds: f64) -> String {
    let s = seconds.round() as u64;
    let (h, m, sec) = (s / 3600, s % 3600 / 60, s % 60);
    let part = |n: u64, unit: &str| format!("{n} {unit}{}", if n == 1 { "" } else { "s" });
    let mut out = Vec::new();
    if h > 0 {
        out.push(part(h, "hour"));
    }
    if m > 0 {
        out.push(part(m, "minute"));
    }
    if sec > 0 || out.is_empty() {
        out.push(part(sec, "second"));
    }
    out.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timer(t: &str) -> Option<(f64, Option<String>)> {
        match parse(t) {
            Intent::Timer { seconds, label } => Some((seconds, label)),
            _ => None,
        }
    }

    #[test]
    fn timers() {
        assert_eq!(timer("start a 25 minute timer"), Some((1500.0, None)));
        assert_eq!(timer("25 min"), Some((1500.0, None)));
        assert_eq!(timer("timer for 10 minutes"), Some((600.0, None)));
        assert_eq!(timer("set a 1 hour timer for deep work"), Some((3600.0, Some("deep work".into()))));
        assert_eq!(timer("90 seconds"), Some((90.0, None)));
        assert_eq!(timer("Start a 5m timer."), Some((300.0, None)));
        assert_eq!(timer("what is 25 minutes in seconds"), None);
    }

    #[test]
    fn the_rest() {
        assert_eq!(parse("stop"), Intent::Stop);
        assert_eq!(parse("Pause the timer"), Intent::Pause);
        assert_eq!(parse("resume"), Intent::Resume);
        assert_eq!(parse("start stopwatch"), Intent::Stopwatch { label: None });
        assert_eq!(
            parse("stopwatch for reading"),
            Intent::Stopwatch {
                label: Some("reading".into())
            }
        );
        assert_eq!(parse("open spotify and play lofi"), Intent::Agent);
        assert_eq!(human(1500.0), "25 minutes");
        assert_eq!(human(90.0), "1 minute 30 seconds");
        assert_eq!(human(3600.0), "1 hour");
    }
}
