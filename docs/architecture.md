# Architecture

Ultron is one Rust workspace. The core has no UI, so every rule in it can be tested and
checked against the original without a screen.

```
crates/
  core/       routing, memory, sessions, tasks, IPC — no UI, no I/O beyond SQLite and files
  app/        the `ultron-rs` binary: CLI front door today, the Tauri app from wave 3
  (planned)
  llm/        one `Llm` trait, one module per provider
  control/    input + screenshots + accessibility, one module per OS
  voice/      Whisper push-to-talk, Gemini Live
  abilities/  WebAssembly sandbox for self-written abilities
  hands/      MCP server exposing `control` to coding agents
ui/           HTML/CSS/JS for the island, prompt bar, companion and dashboard (wave 3)
rust/         the port plan: DECISIONS, MAP, GLOSSARY, HANDOFF, research
```

## Request flow

1. A hotkey runs `ultron-rs --prompt`. The front door sends `prompt\n` to the running Ultron over
   a local socket. If none is running it starts one, guarded by a lock file so there is only ever one.
2. The prompt bar sends the text to the **router**. Skills declare patterns; the first unambiguous
   match wins. Opening apps uses fuzzy matching against the installed-apps index.
3. If nothing matches, or a skill says the request is unclear, the **LLM provider** is asked to
   rewrite it into a skill command or answer in one sentence. It is never given a shell.
4. The skill runs and its reply is shown in the **island**.

## Data

Everything stays on the machine, in the platform's data and config folders:

| File | Holds |
| --- | --- |
| `ultron.db` (SQLite) | timer/stopwatch sessions and segments, memory, tasks |
| `preferences.json` | UI and behavior settings |
| `abilities/` | self-written abilities and their data |

The formats match the original app's exactly (ADR-005), so both versions can read the same data
during the switch.

## Platforms

Platform code sits behind traits with one module per OS:

| Concern | Linux | Windows | macOS |
| --- | --- | --- | --- |
| Single instance IPC | Unix socket | named pipe | Unix socket |
| Input | enigo | enigo | enigo |
| Screenshots | xcap | xcap | xcap |
| Accessibility tree | AT-SPI | UI Automation | AX API |
| UI | Tauri (WebKitGTK) | Tauri (WebView2) | Tauri (WKWebView) |
