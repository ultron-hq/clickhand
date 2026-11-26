# Glossary

| Original (Python) | Meaning | Rust name |
| --- | --- | --- |
| Brain (brain.py) | text → skill → reply, no UI | `core::brain::Brain` |
| skill (skills/*.py) | one ability with MATCH regex + handler | `trait Skill`, `core::skills::*` |
| ROUTES / MATCH / ONLY_MATCH | regex routing table | `core::router::Route` |
| Unclear | skill can't parse the request → ask the LLM | `SkillError::Unclear` |
| llm.py second opinion | LLM rewrites into a skill command or one-line answer | `core::llm::Llm` (ADR-006) |
| memory: facts, aliases, plans | what the user taught Ultron + learned plans | `core::memory::{Fact, Alias, Plan}` |
| session / segment | one timer or stopwatch run / a contiguous stretch of it | `core::sessions::{Session, Segment}` |
| island | flip-clock dynamic island window | `ui/island` (webview) + `app::island` |
| prompt bar | Ctrl+Space input window | `ui/prompt` + `app::prompt` |
| companion | click-through cursor buddy | `ui/companion` + `app::companion` |
| origin == 'user' | request typed by the user (gates destructive actions) | `Origin::User` |
| ability (generated) | user-requested tool Ultron writes itself | `abilities::Ability` (WASM, ADR-008) |
| hands | MCP server letting Claude control the desktop | `hands` crate (ADR-011) |
| ipc / lock | single instance + `ultron --prompt` messages | `app::single_instance` |
