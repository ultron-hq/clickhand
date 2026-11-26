//! Preferences, Gemini settings and the /model /effort /speed /settings commands,
//! ported from ultron/preferences.py, live_config.py and model_settings.py.
//! Same files, same keys, same messages. Parsing and status never call a model.
use std::path::PathBuf;

use serde_json::{json, Map, Value};

/// Where the config files live; `env` reads environment variables (injectable for tests).
pub struct Config<'a> {
    pub data_dir: PathBuf,
    pub gemini: PathBuf,
    pub env: &'a dyn Fn(&str) -> Option<String>,
}

pub fn system_env(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

impl Default for Config<'static> {
    fn default() -> Self {
        Config {
            data_dir: crate::paths::data_dir(),
            gemini: crate::paths::config_dir().join("gemini.json"),
            env: &system_env,
        }
    }
}

pub const MODELS: [&str; 3] = ["haiku", "sonnet", "opus"];
pub const EFFORTS: [&str; 3] = ["low", "medium", "high"];
pub const VOICES: [&str; 5] = ["Charon", "Puck", "Kore", "Aoede", "Fenrir"];
pub const DEFAULT_GEMINI_MODEL: &str = "gemini-3.8-live";
pub const DEFAULT_VOICE: &str = "Charon";

/// The original's defaults, in its key order.
pub fn defaults() -> Map<String, Value> {
    let v = json!({"model":"sonnet","persistent_cursor":false,"voice_language":"auto","voice_auto_send":true,"voice_provider":"gemini","effort":"low","routing_model":"haiku"});
    v.as_object().unwrap().clone()
}

// ── preferences ───────────────────────────────────────────────────────────────

impl Config<'_> {
    fn prefs_path(&self) -> PathBuf {
        self.data_dir.join("preferences.json")
    }

    /// Saved preferences over the defaults; unknown keys and unreadable files are ignored.
    pub fn read(&self) -> Map<String, Value> {
        let mut out = defaults();
        let stored = std::fs::read_to_string(self.prefs_path())
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok());
        if let Some(Value::Object(stored)) = stored {
            for (k, v) in stored {
                if out.contains_key(&k) {
                    out.insert(k, v);
                }
            }
        }
        out
    }

    pub fn save(&self, values: &Map<String, Value>) -> Result<Map<String, Value>, String> {
        let d = defaults();
        if values.keys().any(|k| !d.contains_key(k)) {
            return Err("Unknown preference.".into());
        }
        let one_of = |key: &str, allowed: &[&str], msg: &str| -> Result<(), String> {
            match values.get(key) {
                Some(v) if !v.as_str().is_some_and(|s| allowed.contains(&s)) => Err(msg.into()),
                _ => Ok(()),
            }
        };
        let boolean = |key: &str, msg: &str| -> Result<(), String> {
            match values.get(key) {
                Some(v) if !v.is_boolean() => Err(msg.into()),
                _ => Ok(()),
            }
        };
        one_of("model", &MODELS, "Choose Haiku, Sonnet, or Opus.")?;
        one_of("effort", &EFFORTS, "Choose low, medium, or high effort.")?;
        one_of("routing_model", &MODELS, "Choose Haiku, Sonnet, or Opus for routing.")?;
        boolean("persistent_cursor", "Invalid cursor preference.")?;
        one_of("voice_provider", &["gemini", "local"], "Choose Gemini Live or local dictation.")?;
        one_of(
            "voice_language",
            &["auto", "en", "ar"],
            "Choose automatic, English, or Arabic recognition.",
        )?;
        boolean("voice_auto_send", "Invalid voice submission preference.")?;
        let mut data = self.read();
        for (k, v) in values {
            data.insert(k.clone(), v.clone());
        }
        crate::paths::write_private(&self.prefs_path(), Value::Object(data.clone()).to_string().as_bytes()).map_err(|e| e.to_string())?;
        Ok(data)
    }

    // ── Gemini settings (live_config.py) ──

    /// API key (environment first), model and voice.
    pub fn gemini(&self) -> Map<String, Value> {
        let data = std::fs::read_to_string(&self.gemini)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();
        let env_key = (self.env)("GEMINI_API_KEY")
            .filter(|k| !k.is_empty())
            .or_else(|| (self.env)("GOOGLE_API_KEY").filter(|k| !k.is_empty()));
        let mut out = Map::new();
        out.insert(
            "api_key".into(),
            env_key
                .map(Value::String)
                .unwrap_or_else(|| data.get("api_key").cloned().unwrap_or(json!(""))),
        );
        out.insert("model".into(), data.get("model").cloned().unwrap_or(json!(DEFAULT_GEMINI_MODEL)));
        out.insert("voice".into(), data.get("voice").cloned().unwrap_or(json!(DEFAULT_VOICE)));
        out
    }

    pub fn save_gemini(&self, api_key: &str, model: &str, voice: &str) -> Result<(), String> {
        let (api_key, model, voice) = (api_key.trim(), model.trim(), voice.trim());
        if api_key.is_empty() || api_key.chars().any(char::is_whitespace) {
            return Err("Enter a valid Gemini API key.".into());
        }
        if model.is_empty() || model.chars().count() > 120 || !model.chars().all(|c| c.is_alphanumeric() || "-._/".contains(c)) {
            return Err("Enter a Gemini Live model ID.".into());
        }
        if !VOICES.contains(&voice) {
            return Err("Choose a supported voice.".into());
        }
        let body = json!({"api_key": api_key, "model": model, "voice": voice}).to_string();
        crate::paths::write_private(&self.gemini, body.as_bytes()).map_err(|e| e.to_string())
    }

    // ── model settings ──

    fn effective(&self, saved: &Map<String, Value>) -> Map<String, Value> {
        let mut out = saved.clone();
        if let Some(m) = (self.env)("ULTRON_DESKTOP_MODEL").filter(|m| !m.is_empty()) {
            out.insert("model".into(), json!(m));
        }
        out
    }

    pub fn status(&self, saved: Option<&Map<String, Value>>) -> String {
        let saved = saved.cloned().unwrap_or_else(|| self.read());
        let current = self.effective(&saved);
        let s = |m: &Map<String, Value>, k: &str| {
            m.get(k)
                .map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()))
                .unwrap_or_default()
        };
        let voice = if s(&saved, "voice_provider") == "gemini" {
            s(&self.gemini(), "model")
        } else {
            "Whisper small (local dictation)".into()
        };
        let overridden = (self.env)("ULTRON_DESKTOP_MODEL").is_some_and(|m| !m.is_empty());
        let over = if overridden { " (ULTRON_DESKTOP_MODEL override)" } else { "" };
        let effort = if s(&current, "model") != "haiku" {
            s(&current, "effort")
        } else {
            "not applied on Haiku".into()
        };
        format!(
            "Desktop: {}{over}\nEffort: {effort}\nRouting fallback: {}\nVoice: {voice}\nLaya: local routing, 2-second request limit.\nUse /model, /effort, /speed fast|balanced|thorough. Changes apply to the next task.",
            s(&current, "model"),
            s(&saved, "routing_model"),
        )
    }

    /// /model, /effort, /speed, /settings. None when the text isn't one of them.
    pub fn handle(&self, text: &str, dry_run: bool) -> Result<Option<String>, String> {
        let lower = text.trim().to_lowercase();
        let parts: Vec<&str> = lower.split_whitespace().collect();
        let Some(&command) = parts.first() else { return Ok(None) };
        let mut args: Vec<&str> = parts[1..].to_vec();
        let saved = self.read();
        let mut change: Vec<(&str, &str)> = Vec::new();
        match command {
            "/settings" => {
                return Ok(Some(if args.is_empty() {
                    self.status(Some(&saved))
                } else {
                    "Usage: /settings".into()
                }))
            }
            "/model" => {
                let mut field = "model";
                if args.first().is_some_and(|a| *a == "router" || *a == "desktop") {
                    field = if args.remove(0) == "router" { "routing_model" } else { "model" };
                }
                if args.is_empty() {
                    return Ok(Some(format!(
                        "{}\nModels: haiku, sonnet, opus. Example: /model sonnet or /model router haiku.",
                        self.status(Some(&saved))
                    )));
                }
                if args.len() != 1 || !MODELS.contains(&args[0]) {
                    return Ok(Some("Usage: /model [desktop|router] haiku|sonnet|opus".into()));
                }
                change.push((field, args[0]));
            }
            "/effort" => {
                if args.is_empty() {
                    let e = saved.get("effort").and_then(Value::as_str).unwrap_or("low");
                    return Ok(Some(format!(
                        "Saved desktop effort: {e}. Choose /effort low|medium|high. Haiku ignores effort."
                    )));
                }
                if args.len() != 1 || !EFFORTS.contains(&args[0]) {
                    return Ok(Some("Usage: /effort low|medium|high".into()));
                }
                change.push(("effort", args[0]));
            }
            "/speed" => {
                if args.is_empty() {
                    return Ok(Some(format!(
                        "Presets: /speed fast (Haiku/low), balanced (Sonnet/medium), thorough (Opus/high).\n{}",
                        self.status(Some(&saved))
                    )));
                }
                let preset = match (args.len(), args[0]) {
                    (1, "fast") => [("model", "haiku"), ("effort", "low")],
                    (1, "balanced") => [("model", "sonnet"), ("effort", "medium")],
                    (1, "thorough") => [("model", "opus"), ("effort", "high")],
                    _ => return Ok(Some("Usage: /speed fast|balanced|thorough".into())),
                };
                change.extend(preset);
            }
            _ => return Ok(None),
        }
        if dry_run {
            let list: Vec<String> = change.iter().map(|(k, v)| format!("{k}={v}")).collect();
            return Ok(Some(format!("Would update: {}", list.join(", "))));
        }
        let values: Map<String, Value> = change.iter().map(|(k, v)| (k.to_string(), json!(v))).collect();
        let saved = self.save(&values)?;
        Ok(Some(format!("Saved for the next task.\n{}", self.status(Some(&saved)))))
    }
}

pub fn is_command(text: &str) -> bool {
    text.split_whitespace()
        .next()
        .is_some_and(|w| ["/model", "/effort", "/speed", "/settings"].contains(&w.to_lowercase().as_str()))
}

/// CLI flags for the Claude Code CLI; Haiku has no effort dial.
pub fn claude_options(model: &str, effort: &str) -> Vec<String> {
    let mut out = vec!["--model".to_string(), model.to_string()];
    if model != "haiku" {
        out.extend(["--effort".to_string(), effort.to_string()]);
    }
    out
}

/// Environment for the Claude Code CLI: let adaptive reasoning follow the chosen effort.
pub fn claude_env(model: &str, base: impl IntoIterator<Item = (String, String)>) -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = base
        .into_iter()
        .filter(|(k, _)| k != "CLAUDE_CODE_EFFORT_LEVEL" && k != "MAX_THINKING_TOKENS")
        .collect();
    if model == "haiku" {
        env.push(("MAX_THINKING_TOKENS".into(), "0".into()));
    }
    env
}
