# Changelog

All notable changes are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.3.3] - 2026-11-28

### Changed
- The app follows each OS. macOS: ⌥ Space by default (Ctrl+Space is the Mac's input-source switch), Mac key symbols, frosted glass behind the bar and island, SF fonts, a monochrome menu-bar icon, a see-through title bar on the welcome window. Windows: Windows 11 corners and Segoe UI Variable, click the tray icon to ask (right-click for the menu), and the welcome window shows how to pin it to the taskbar. Linux: the desktop's font, and a Wayland warning in the welcome window.
- The bar: suggestions when empty, a footer with who answers and the keys, a scanning line and an elapsed counter while working, and a window exactly as tall as its content.
- The island: a ring that drains with a timer; "working" while the agent works, then "✓ answered".

### Fixed
- Windows: asking the AI from the bar failed ("batch file arguments are invalid") when Claude Code or Codex was installed with npm. The prompt now goes in on stdin.

## [0.3.2] - 2026-11-28

### Fixed
- Windows: asking the app something no longer opens a Codex or Claude Code console that steals focus and hides the bar; they run in the background (the whole launch chain, including `.cmd` wrappers), and so does "Connect my AI apps".
- Typing can't silently land in the wrong window: `type_text` and `key` take a `window_id` and refuse unless it has focus, and typing stops (saying how much got through) if focus moves (Linux and Windows).
- `focus_window` works on Windows (restores minimized windows, gets past the foreground lock, verifies). On macOS it says up front that it isn't supported yet.

### Changed
- The prompt bar: Esc only hides it and a running request keeps going; reopening shows it working, answered or cancelled. Cancel (button or Ctrl+.) stops it, process tree included, and a late answer can't replace newer output. After you submit, the composer is empty and your prompt sits above its answer; ↑ brings it back to edit or retry.

## [0.3.1] - 2026-11-27

### Fixed
- The kill switch interrupts an action already running: typing stops within a few characters (it used to finish the whole text), and drags, scrolls and multi-clicks stop too.
- Setup no longer turns on "act without asking" by itself: `--yes`, the app and setups without a terminal leave it off. Opt in by answering "y" or with `openhud setup --approve`.
- Windows installer: a reinstall now fixes the current terminal's PATH even when the saved PATH was already right.
- Codex's `config.toml` (and Claude Code's `~/.claude.json`) get a one-time `.bak` before any change, like the JSON configs.

### Added
- Discord: https://discord.gg/32uFQ8PUQB

## [0.3.0] - 2026-11-27

### Changed
- Renamed to **OpenHUD** (openhud.ai): the `openhud` command and MCP server, the OpenHUD app, and the `/openhud`, `/openhud-teach` and `/openhud-post` skills. New HUD reticle logo.
- Upgrades keep working: setup replaces `clickhand` and `ultron-hands` agent entries our installer made, removes the old setup-installed skills, and the installers leave `clickhand` and `ultron-hands` aliases.

## [0.2.2] - 2026-11-27

### Fixed
- Linux: the island is its real size (an invisible strip under the clock was blocking clicks).
- The welcome window's "Start using OpenHUD" button no longer wraps.
- The Mac installer falls back to the newest release that has the app while a new one is still building.

## [0.2.1] - 2026-11-27

### Changed
- The new OpenHUD icon in the app (Dock, tray, window) and the disk image.
- The Mac disk image opens to a laid-out window: OpenHUD, an arrow, and Applications, over "Drag OpenHUD into Applications" (built with dmgbuild, which works on CI).
- The welcome window fits small Mac screens.

## [0.1.5] - 2026-11-27

### Added
- **The OpenHUD app** (start of wave 3), for Windows, macOS and Linux: a flip-clock island at the top of the screen, a Ctrl+Space prompt bar and a tray icon. Timers, stopwatch, pause, resume and stop are answered instantly and logged to the sessions database; anything else goes to Claude Code or Codex with OpenHUD.
- First-run welcome: on macOS it offers to move OpenHUD into Applications (and clears the download quarantine), then shows the shortcut, connects your AI apps with the bundled OpenHUD in one click, opens the Screen Recording and Accessibility settings, and can start OpenHUD at login. Reopen it from the tray.
- Installers on every release: a drag-to-Applications `.dmg` for Apple silicon and Intel Macs, a Windows setup `.exe`, and a Linux `.AppImage` and `.deb`, each with OpenHUD inside.
- Teach highlights and the "AI is using your mouse" overlay on macOS.

## [0.1.4] - 2026-11-27

### Added
- The screen frame, pointer tag and Teach highlights now work on Windows too, kept out of the agent's screenshots by Windows itself (WDA_EXCLUDEFROMCAPTURE).
- `openhud-post` skill: write and publish posts, replies and threads on X, LinkedIn, Threads and Bluesky from your own logged-in browser. It drafts first, checks the length, attaches media before typing, verifies the post went up, and posts only when you asked.
- `type_text` takes `delay_ms` to type one character at a time, for sites that drop fast keystrokes.

## [0.1.3] - 2026-11-27

### Added
- You can see when an agent is using your computer: an orange frame around the screen while it works, and an "OpenHUD is using your mouse" tag beside the pointer when it clicks or types. Both are click-through and hidden from the agent's screenshots (Linux/X11 first; `OPENHUD_OVERLAY=0` turns it off).
- OpenHUD Teach: a `highlight` tool draws a box and caption on your screen ("Click Export here") without clicking anything, so an agent can show you how to use an app while you do the steps. Works in read-only mode.
- Agent skills: setup installs `openhud` and `openhud-teach` into Claude Code and Codex, so `/openhud-teach show me how to ... in Figma` works. `openhud skills` reinstalls them; copies you edit are left alone.

## [0.1.2] - 2026-11-27

### Fixed
- Screenshots no longer fail at random when Hands is the X server's only client (Ubuntu 22.04, Debian 12, openSUSE with Xvfb).
- macOS: clicks land where asked. The pointer is warped to the target, and on every OS a click only happens once the pointer is confirmed there.
- `openhud log | head` no longer panics.

### Changed
- Linux binaries are static (musl) and run on any distro, Alpine included.

### Added
- End-to-end tests: on 7 Linux distros in Docker (`tests/e2e/linux.sh`) and on real Windows, macOS (Apple silicon, Intel) and Ubuntu desktops (`e2e` workflow, also run against the live installer after every release).

## [0.1.1] - 2026-11-27

### Fixed
- Setup now connects Codex when `CODEX_HOME` points to a folder that doesn't exist yet.

## [0.1.0] - 2026-11-27

### Added
- One-line install for OpenHUD: `curl -fsSL https://openhud.vercel.app/install.sh | sh` (macOS, Linux) and `irm https://openhud.vercel.app/install.ps1 | iex` (Windows). It verifies the SHA-256 of the download and adds the binary to your PATH.
- Prebuilt `openhud` binaries for Linux (x86_64, arm64), macOS (Apple silicon, Intel) and Windows, published by a release workflow on every `v*` tag.
- Guided setup: run `openhud` in a terminal to check the screen, input and permissions (it opens the macOS privacy settings when needed) and connect Claude Code, Codex, Cursor, VS Code, Windsurf, Claude Desktop and Gemini CLI. Existing configs are merged, backed up once, and never overwritten without asking.
- `openhud stop` / `resume` (kill switch), `log`, `doctor`, `agents`, `connect <agent>`, `disconnect <agent>`.
- `ultron-rs` command line front door with the original's flags: `--prompt`, `--cursor`, `--voice`, `--say`, `--quit`, `--keepalive`.
- Single-instance lock and the line protocol for talking to a running OpenHUD (Unix socket, named pipe on Windows).
- CI on Linux, Windows and macOS (Apple silicon and Intel).
- Port plan: decisions (ADR-001 to ADR-012), unit map and glossary under `rust/`.
