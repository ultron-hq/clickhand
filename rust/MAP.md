# Ultron → Rust: port map

**Goal:** one Rust app (Tauri) that does what Python Ultron does, on Windows, macOS and Linux. The Python original is the reference (kept locally in `reference/`, not published) and is never edited; it stays the daily driver until cutover (ADR-004).

Decisions: [DECISIONS.md](DECISIONS.md) — ADR-001 Tauri · 012 open source · 013 own recorder per unit · 014 personal integrations off ·  · 002 drop Laya · 003 whisper-rs + Gemini · 004 side by side · 005 same data formats · 006 pluggable LLMs · 007 native control per OS · 008 WASM abilities · 009 three platforms · 010 equivalence bar · 011 hands MCP in Rust.
Terms: [GLOSSARY.md](GLOSSARY.md). Next session: read this, then HANDOFF.md.

States: `open` · `claimed by <session> <date>` · `done (gate, replay)`.

## Wave 0 — skeleton
| Unit | State |
| --- | --- |
| Cargo workspace: `core`, `app` (more crates added as their wave starts) | done (2026-11-26) |
| CI: build + test on ubuntu, windows, macos (arm64 + x86_64); installers on tag | done (build+test); installers with wave 3 |
| Single instance + `ultron --prompt/--cursor/--voice/--quit/--say/--keepalive` CLI (ipc.py, __main__.py) | done: core::ipc tests pass, --say verified against live Python Ultron |

## Wave 1 — pure logic (recorded, must match; ADR-010)
| Unit | Original | State |
| --- | --- | --- |
| Config + preferences + paths | preferences.py, live_config.py, model_settings.py | done (2026-11-26): `core::config`, `core::paths` (Linux = original paths, platform dirs elsewhere). /settings still mentions Laya, as the original does: reword with ADR-006 provider settings |
| Sessions store + reports | sessions.py, reports.py | done (2026-11-26): `core::sessions`, `core::reports`; 540 recorded cases replay (tests/recorded.rs), mutation-checked. HTML dashboard waits for tasks/runs/abilities |
| Memory (facts, aliases, plans, corrections) | memory.py | done (2026-11-26): `core::memory`; 190 recorded cases with tasks, template regex keys byte-identical to Python's (shared DB), aliases via fancy-regex; 3 mutation checks |
| Tasks | tasks.py | done (2026-11-26): `core::tasks` (validate incl. all six ISO date forms Python 3.13 accepts, create/update/list, `link_session`). `focus()` moves to wave 3 (needs the island) |
| Router: regex routes + skill MATCH table | brain.py, skills/__init__.py (static registry replaces importlib) | done (2026-11-26): `core::router`; registry extracted to router/skills.json in the original's order; skills + planner are traits; 349 cases with config, 3 mutation checks. `handle()` waits for commands.py; desktop follow-ups for wave 4 |
| App index + fuzzy open_app (replaces Laya) | apps_index.py | open |
| Workflow selection | workflows.py, paper.py | open |
| Commands | commands.py | done (2026-11-27): `core::commands` (slash commands, @ tags, build requests, autocomplete) + `Router::handle`; personal integrations off by default (ADR-014); 108 cases, 3 mutation checks |
| Annotations / identity | annotations.py, identity.py | open |

## Wave 2 — LLM + skills
| Unit | State |
| --- | --- |
| `llm` crate: Claude CLI, Codex CLI, Ollama/OpenAI-compatible, API providers (ADR-006) | open |
| Skills: timer, clock, volume, search, apps, system, deen, reports, memory, tasks, chat, screenshot, screen | open |
| Skills: deploy, workspace, agents, hacker, workflow, companion_settings | open |
| Companion web-app link (optional integration) | open |

## Wave 3 — UI (checked by eye + screenshots)
| Unit | Original | State |
| --- | --- | --- |
| Island + flip clock (timer/stopwatch hooks → sessions) | island.py, flipclock.py, glass.py | open |
| Prompt bar (centred / beside cursor) | prompt.py | open |
| Companion cursor + overlay | companion.py, overlay.py, pointer.py, mouse.py | open |
| Dashboard + abilities UI | dashboard.py, abilities.py, abilities_ui.js, report.html | open |
| Tray, autostart, global hotkeys | app.py | open |

## Wave 4 — desktop control
| Unit | State |
| --- | --- |
| `control` crate: enigo + xcap + A11y trait (AT-SPI / UIA / AX) | open |
| Desktop task loop (observe → act → verify) | open |
| hands MCP server (rmcp), same tool names | open |

## Wave 5 — voice
| Unit | State |
| --- | --- |
| whisper-rs push-to-talk (voice.py, voice_worker.py) | open |
| Gemini Live over WebSocket (live_*.py) | open |

## Wave 6 — abilities + team
| Unit | State |
| --- | --- |
| WASM abilities in wasmtime + builder; legacy Python/bubblewrap on Linux | open |
| Team / runs / tasks journal | open |

## Wave 7 — cutover
| Unit | State |
| --- | --- |
| rust/cutover.sh: switch hotkey + autostart to Rust, and back | open |

## Out of scope
- Laya model (ADR-002).
- ultron-workspace (AGPL, separate app).

## Open questions
- none yet
