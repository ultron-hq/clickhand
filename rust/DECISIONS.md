# Ultron → Rust: decisions

Source of truth for the port. Never renumber; a changed decision gets a new ADR that supersedes the old one.

## ADR-001: Pure Rust desktop app on Tauri
Status: accepted (2026-11-26)
Context: Ultron is a PyQt6 desktop app (island, prompt bar, companion cursor, dashboard). rust-anything cannot port a GUI; the goal is one app that runs on Windows, macOS and Linux.
Decision: Tauri 2. Rust core owns everything; the island, prompt bar, companion and dashboard are HTML/CSS/JS webviews (transparent, always-on-top). Global hotkeys via tauri-plugin-global-shortcut, tray via Tauri tray.
Consequences: The flip clock and liquid-glass look are rebuilt in CSS. UI is checked by eye and screenshots, not by recordings. Logic units are checked against recordings of the Python original.

## ADR-002: Drop the Laya model
Status: accepted (2026-11-26)
Context: Laya (PyTorch) only chooses open_app / web_search / none and routed ~28/35; PyTorch does not ship well on all three platforms.
Decision: No Laya. Regex routes stay; open_app uses fuzzy matching against the installed-apps index; anything unsure goes to the LLM (ADR-006), as today.
Consequences: No torch, no model download; the binary stays small. Recorded routing cases for open_app/web_search may differ where Laya decided: those cases are re-baselined, not matched.

## ADR-003: Voice: whisper-rs locally, Gemini Live over WebSocket
Status: accepted (2026-11-26)
Decision: faster-whisper → whisper-rs (whisper.cpp, CPU, all platforms; model downloaded on first use). Gemini Live (google-genai) → its WebSocket API called from Rust (tokio-tungstenite). Audio I/O via cpal.
Consequences: Transcripts may differ slightly from faster-whisper; voice is checked by behavior, not byte equality.

## ADR-004: Side-by-side rollout
Status: accepted (2026-11-26)
Decision: The Rust app is built in its own repository. The Python original is never modified and stays on Ctrl+Space until the Rust app passes its recorded checks; then one command switches the hotkey/autostart, and one switches back.
Consequences: Both must be able to read the same data, so on-disk formats are frozen (ADR-005). Only one runs at a time (shared lock).

## ADR-005: Keep data and config formats
Status: accepted (2026-11-26)
Decision: Same files, same schemas: ~/.local/share/ultron/ultron.db (sessions, segments, memory, tasks), preferences.json, dashboard.json/token, abilities/; ~/.config/ultron/*.json|.env|.conf. Paths map to the platform's data/config dirs (directories crate) on Windows/macOS, identical paths on Linux.
Consequences: SQLite via rusqlite (bundled). Schema changes need a new ADR.

## ADR-006: Pluggable LLM providers
Status: accepted (2026-11-26)
Context: Today Ultron shells out to `claude -p` (Haiku for routing, configurable model for desktop/team tasks).
Decision: One `Llm` trait with providers: Claude Code CLI, Codex CLI, local model (Ollama / any OpenAI-compatible endpoint), and API key for any provider (Anthropic, OpenAI, Gemini, OpenRouter, ...). Chosen per role (router, desktop agent, team) in settings; Claude CLI stays the default so current behavior is unchanged.
Consequences: Prompts and the "rewrite into a skill command or answer in one sentence, never shell" contract are shared across providers. Keys live in the OS keychain (keyring crate), never in plain config.

## ADR-007: Desktop control native per OS
Status: accepted (2026-11-26)
Decision: enigo (mouse/keyboard) and xcap (screenshots) on all platforms; accessibility tree via AT-SPI (atspi crate) on Linux, UI Automation (uiautomation crate) on Windows, AX API (accessibility-sys) on macOS, behind one `A11y` trait. Safety rules carry over unchanged: destructive actions only when the user asked (origin == user), kill switch, action log.
Consequences: xdotool/scrot/wmctrl dependencies disappear. macOS needs Accessibility + Screen Recording permission prompts.

## ADR-008: Generated abilities run as WebAssembly
Status: accepted (2026-11-26)
Decision: New generated abilities compile to WASM (WASI) and run in wasmtime with capability grants mirroring today's manifest (network, folders, desktop). Existing Python abilities keep running in bubblewrap on Linux only.
Consequences: The ability builder prompt changes to emit Rust (or another WASI language). Same isolation on every OS, no Python needed.

## ADR-009: All three platforms from the start
Status: accepted (2026-11-26)
Decision: Linux (x86_64), Windows (x86_64), macOS (arm64 + x86_64) built and tested in GitHub Actions on every push; installers: .AppImage/.deb, .msi/.exe, .dmg. Recordings are made on Linux (the original only runs there); platform-specific code is covered by per-OS tests.
Consequences: Every unit must compile on all three before it is marked done.

## ADR-010: Equivalence bar
Status: accepted (2026-11-26)
Decision: Logic units (routing, memory, sessions/reports, tasks, workflows selection, parsing) must match recordings exactly after normalizing timestamps, ids and dict order. LLM output, voice and UI are checked by behavior, not bytes.
Consequences: Calls to the LLM, desktop and network are stubbed during record/replay (no side effects run while recording).

## ADR-011: hands MCP server ported with the app
Status: accepted (2026-11-26)
Decision: ultron/hands/server.py (Python mcp + PIL) becomes a Rust stdio MCP server (rmcp crate) in the same workspace, sharing the ADR-007 control layer. Tool names and arguments stay identical so Claude Code config does not change.

## ADR-012: Open source under ultron-hq
Status: accepted (2026-11-26)
Decision: The Rust port is public at github.com/ultron-hq/ultron, licensed MIT OR Apache-2.0 (the Rust ecosystem default; ADR-001's MIT is superseded on licensing only). The Python original stays private and is not part of the repository; recordings made from it are kept out of git when they contain personal data.
Consequences: No personal paths, accounts, tokens or recordings in the repo. Contributions are accepted under the same dual license.
