# Changelog

All notable changes are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.1] - 2026-11-27

### Fixed
- Setup now connects Codex when `CODEX_HOME` points to a folder that doesn't exist yet.

## [0.1.0] - 2026-11-27

### Added
- One-line install for Ultron Hands: `curl -fsSL https://getultron.vercel.app/install.sh | sh` (macOS, Linux) and `irm https://getultron.vercel.app/install.ps1 | iex` (Windows). It verifies the SHA-256 of the download and adds the binary to your PATH.
- Prebuilt `ultron-hands` binaries for Linux (x86_64, arm64), macOS (Apple silicon, Intel) and Windows, published by a release workflow on every `v*` tag.
- Guided setup: run `ultron-hands` in a terminal to check the screen, input and permissions (it opens the macOS privacy settings when needed) and connect Claude Code, Codex, Cursor, VS Code, Windsurf, Claude Desktop and Gemini CLI. Existing configs are merged, backed up once, and never overwritten without asking.
- `ultron-hands stop` / `resume` (kill switch), `log`, `doctor`, `agents`, `connect <agent>`, `disconnect <agent>`.
- `ultron-rs` command line front door with the original's flags: `--prompt`, `--cursor`, `--voice`, `--say`, `--quit`, `--keepalive`.
- Single-instance lock and the line protocol for talking to a running Ultron (Unix socket, named pipe on Windows).
- CI on Linux, Windows and macOS (Apple silicon and Intel).
- Port plan: decisions (ADR-001 to ADR-012), unit map and glossary under `rust/`.
