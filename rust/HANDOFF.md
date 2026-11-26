# Handoff — 2026-11-26

Done this session:
- Grilling finished: ADR-001..011 in DECISIONS.md, plan in MAP.md, GLOSSARY.md started.
- `rust-anything analyze . --whole-app` run (rust-anything 5.0.1; needs rustc ≥ 1.97).
- Wave 0: Cargo workspace (`crates/core`, `crates/app`), `core::ipc` (single instance + line protocol, same socket/lock paths as Python), `ultron-rs` front door CLI (all flags; starting falls back to the Python launcher (`$ULTRON_ORIGINAL`)). Verified `ultron-rs --say` reaches the live Python Ultron.
- CI for Linux / Windows / macOS arm64 + x86_64 (.github/workflows/ci.yml); not pushed anywhere yet.

Next:
1. Tauri needs `libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev` (installed with the system package manager).
2. Wave 1 units, starting with sessions + reports (pure SQLite, easy to record with a copy of ultron.db).
3. Never run `app record` on flags with side effects (--quit, --prompt) against the live Ultron: record with a stub instance.
