# Handoff — 2026-11-26 (wave 1: 3 units done)

Done:
- Wave 0 (see MAP). Repo public at github.com/ultron-hq/ultron, site at getultron.vercel.app (website/).
- Wave 1, unit 1: sessions + reports. `crates/core/src/{sessions,reports}.rs`. Recorded with `rust/record/sessions_reports.py` (ADR-013): 540 cases from the Python original, replayed by `crates/core/tests/recorded.rs`. Mutation checks: breaking fmt rounding and the DST-gap rule both fail the replay.

How to re-record a unit:
    TZ=Europe/Paris PYTHONPATH=reference python3 rust/record/sessions_reports.py
(the Python original lives in `reference/`, git-ignored; copy it from ~/laya/ultron if missing.)

- Wave 1, units 2–3: memory + tasks. `crates/core/src/{memory,tasks}.rs`, recorded by `rust/record/memory_tasks.py` (190 cases), replayed by `tests/memory_tasks.rs`. Mutation checks: escaping, alias lookbehind, correction precedence.

Next:
1. Config/preferences/paths (preferences.py, live_config.py, model_settings.py).
2. Router (brain.py + skills/__init__.py): static registry; watch regex features (fancy-regex where Python uses lookaround).
3. Reports HTML dashboard once tasks/runs/abilities are ported.
4. Tauri still needs the WebKitGTK dev packages on Linux.
