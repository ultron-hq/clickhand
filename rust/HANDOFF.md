# Handoff — 2026-11-26 (wave 1 started)

Done:
- Wave 0 (see MAP). Repo public at github.com/ultron-hq/ultron, site at getultron.vercel.app (website/).
- Wave 1, unit 1: sessions + reports. `crates/core/src/{sessions,reports}.rs`. Recorded with `rust/record/sessions_reports.py` (ADR-013): 540 cases from the Python original, replayed by `crates/core/tests/recorded.rs`. Mutation checks: breaking fmt rounding and the DST-gap rule both fail the replay.

How to re-record a unit:
    TZ=Europe/Paris PYTHONPATH=reference python3 rust/record/sessions_reports.py
(the Python original lives in `reference/`, git-ignored; copy it from ~/laya/ultron if missing.)

Next:
1. Memory (memory.py), then tasks (tasks.py), with the same recorder pattern. Memory touches Haiku-learned plans: stub the LLM in the recorder.
2. Router (brain.py + skills/__init__.py): static registry; watch regex features (fancy-regex where Python uses lookaround).
3. Reports HTML dashboard once tasks/runs/abilities are ported.
4. Tauri still needs the WebKitGTK dev packages on Linux.
