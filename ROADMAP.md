# Roadmap

The rewrite moves in waves. A wave is done when every unit in it passes its recorded checks
against the original and builds on Windows, macOS and Linux. Live status, unit by unit:
[rust/MAP.md](rust/MAP.md).

| Wave | What lands | Status |
| --- | --- | --- |
| 0 | Workspace, CI on three OSes, `ultron` CLI front door, single instance | done |
| 1 | Pure logic: config, sessions + reports, memory, tasks, router, fuzzy app matching, workflows | next |
| 2 | Model providers (Claude Code, Codex, local, API keys) and the skills | |
| 3 | Desktop UI in Tauri: flip-clock island, prompt bar, companion, dashboard, tray, installers | |
| 4 | Desktop control per OS, the desktop task loop, the Hands MCP server | |
| 5 | Voice: Whisper push-to-talk, Gemini Live | |
| 6 | Self-written abilities in a WebAssembly sandbox, agent teams | |
| 7 | Cutover tooling: switch from the Python app and back | |

Why each choice was made: [rust/DECISIONS.md](rust/DECISIONS.md).
