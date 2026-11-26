# Changelog

All notable changes are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- `ultron-rs` command line front door with the original's flags: `--prompt`, `--cursor`, `--voice`, `--say`, `--quit`, `--keepalive`.
- Single-instance lock and the line protocol for talking to a running Ultron (Unix socket, named pipe on Windows).
- CI on Linux, Windows and macOS (Apple silicon and Intel).
- Port plan: decisions (ADR-001 to ADR-012), unit map and glossary under `rust/`.
