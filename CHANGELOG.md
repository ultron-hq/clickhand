# Changelog

All notable changes are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.2.2] - 2026-11-27

### Fixed
- Linux: the island is its real size (an invisible strip under the clock was blocking clicks).
- The welcome window's "Start using Clickhand" button no longer wraps.
- The Mac installer falls back to the newest release that has the app while a new one is still building.

## [0.2.1] - 2026-11-27

### Changed
- The new Clickhand icon in the app (Dock, tray, window) and the disk image.
- The Mac disk image opens to a laid-out window: Clickhand, an arrow, and Applications, over "Drag Clickhand into Applications" (built with dmgbuild, which works on CI).
- The welcome window fits small Mac screens.

## [0.1.5] - 2026-11-27

### Added
- **The Clickhand app** (start of wave 3), for Windows, macOS and Linux: a flip-clock island at the top of the screen, a Ctrl+Space prompt bar and a tray icon. Timers, stopwatch, pause, resume and stop are answered instantly and logged to the sessions database; anything else goes to Claude Code or Codex with Clickhand.
- First-run welcome: on macOS it offers to move Clickhand into Applications (and clears the download quarantine), then shows the shortcut, connects your AI apps with the bundled Clickhand in one click, opens the Screen Recording and Accessibility settings, and can start Clickhand at login. Reopen it from the tray.
- Installers on every release: a drag-to-Applications `.dmg` for Apple silicon and Intel Macs, a Windows setup `.exe`, and a Linux `.AppImage` and `.deb`, each with Clickhand inside.
- Teach highlights and the "AI is using your mouse" overlay on macOS.

## [0.1.4] - 2026-11-27

### Added
- The screen frame, pointer tag and Teach highlights now work on Windows too, kept out of the agent's screenshots by Windows itself (WDA_EXCLUDEFROMCAPTURE).
- `clickhand-post` skill: write and publish posts, replies and threads on X, LinkedIn, Threads and Bluesky from your own logged-in browser. It drafts first, checks the length, attaches media before typing, verifies the post went up, and posts only when you asked.
- `type_text` takes `delay_ms` to type one character at a time, for sites that drop fast keystrokes.

## [0.1.3] - 2026-11-27

### Added
- You can see when an agent is using your computer: an orange frame around the screen while it works, and an "Clickhand is using your mouse" tag beside the pointer when it clicks or types. Both are click-through and hidden from the agent's screenshots (Linux/X11 first; `CLICKHAND_OVERLAY=0` turns it off).
- Clickhand Teach: a `highlight` tool draws a box and caption on your screen ("Click Export here") without clicking anything, so an agent can show you how to use an app while you do the steps. Works in read-only mode.
- Agent skills: setup installs `clickhand` and `clickhand-teach` into Claude Code and Codex, so `/clickhand-teach show me how to ... in Figma` works. `clickhand skills` reinstalls them; copies you edit are left alone.

## [0.1.2] - 2026-11-27

### Fixed
- Screenshots no longer fail at random when Hands is the X server's only client (Ubuntu 22.04, Debian 12, openSUSE with Xvfb).
- macOS: clicks land where asked. The pointer is warped to the target, and on every OS a click only happens once the pointer is confirmed there.
- `clickhand log | head` no longer panics.

### Changed
- Linux binaries are static (musl) and run on any distro, Alpine included.

### Added
- End-to-end tests: on 7 Linux distros in Docker (`tests/e2e/linux.sh`) and on real Windows, macOS (Apple silicon, Intel) and Ubuntu desktops (`e2e` workflow, also run against the live installer after every release).

## [0.1.1] - 2026-11-27

### Fixed
- Setup now connects Codex when `CODEX_HOME` points to a folder that doesn't exist yet.

## [0.1.0] - 2026-11-27

### Added
- One-line install for Clickhand: `curl -fsSL https://clickhand.vercel.app/install.sh | sh` (macOS, Linux) and `irm https://clickhand.vercel.app/install.ps1 | iex` (Windows). It verifies the SHA-256 of the download and adds the binary to your PATH.
- Prebuilt `clickhand` binaries for Linux (x86_64, arm64), macOS (Apple silicon, Intel) and Windows, published by a release workflow on every `v*` tag.
- Guided setup: run `clickhand` in a terminal to check the screen, input and permissions (it opens the macOS privacy settings when needed) and connect Claude Code, Codex, Cursor, VS Code, Windsurf, Claude Desktop and Gemini CLI. Existing configs are merged, backed up once, and never overwritten without asking.
- `clickhand stop` / `resume` (kill switch), `log`, `doctor`, `agents`, `connect <agent>`, `disconnect <agent>`.
- `ultron-rs` command line front door with the original's flags: `--prompt`, `--cursor`, `--voice`, `--say`, `--quit`, `--keepalive`.
- Single-instance lock and the line protocol for talking to a running Clickhand (Unix socket, named pipe on Windows).
- CI on Linux, Windows and macOS (Apple silicon and Intel).
- Port plan: decisions (ADR-001 to ADR-012), unit map and glossary under `rust/`.
