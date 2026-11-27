<p align="center">
  <img src="assets/logo.svg" width="120" height="120" alt="Clickhand">
</p>

<h1 align="center">Clickhand</h1>

<p align="center"><b>Give your AI a hand.</b><br>
Clickhand lets Claude Code, Codex, Cursor or any MCP agent see your screen, move the mouse and type,<br>
so it can do anything that needs a person at the computer, or teach you how while you click.</p>

<p align="center">
  <a href="https://github.com/ultron-hq/ultron/actions/workflows/ci.yml"><img src="https://github.com/ultron-hq/ultron/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <img src="https://img.shields.io/badge/Windows%20·%20macOS%20·%20Linux-supported-2563EB" alt="Windows, macOS and Linux">
  <a href="https://github.com/ultron-hq/ultron/releases/latest"><img src="https://img.shields.io/github/v/release/ultron-hq/ultron?label=release&color=ff7a2f" alt="Latest release"></a>
  <a href="#license"><img src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-555" alt="MIT OR Apache-2.0"></a>
</p>

<p align="center"><a href="https://clickhand.vercel.app">Website</a> · <a href="https://clickhand.vercel.app/docs">Docs</a> · <a href="#install">Install</a> · <a href="ROADMAP.md">Roadmap</a> · <a href="CONTRIBUTING.md">Contributing</a></p>

## Install

| | |
| --- | --- |
| **macOS** (the app, with everything inside) | `curl -fsSL https://clickhand.vercel.app/mac.sh \| sh` |
| **Windows** | `irm https://clickhand.vercel.app/install.ps1 \| iex` |
| **Linux** | `curl -fsSL https://clickhand.vercel.app/install.sh \| sh` |

Or download the `.dmg`, Windows setup or Linux `.AppImage`/`.deb` from the
[latest release](https://github.com/ultron-hq/ultron/releases/latest). Setup connects your AI apps;
on a Mac you allow Screen Recording and Accessibility **once, for Clickhand**, and every AI app works
through it.

Then ask your agent:

```
/clickhand        fill in this form from invoice.pdf
/clickhand-teach  show me how to add RSI on TradingView
/clickhand-post   announce v2 on X with ~/Videos/demo.mp4
```

> [!NOTE]
> Clickhand started as **Ultron**, a personal desktop agent written in Python (a flip-clock island,
> a Ctrl+Space prompt bar, voice, memory). Its Hands, the part that lets an AI use the computer,
> became this project. The rest of that app is being rewritten here in Rust, one unit at a time,
> each checked against the original before it ships with
> [rust-anything](https://github.com/erkamyaman/rust-anything). See the [roadmap](ROADMAP.md).

## How Clickhand works

Claude Code, Codex and other agents can edit files but can't click. **Clickhand** gives them a
mouse and keyboard: an [MCP](https://modelcontextprotocol.io) server that lets any agent see the
screen, click, scroll and type, so you can ask it for anything that needs a person at the computer.

**Install** (one line, no Rust needed):

```sh
curl -fsSL https://clickhand.vercel.app/install.sh | sh          # macOS, Linux
```

```powershell
irm https://clickhand.vercel.app/install.ps1 | iex               # Windows (PowerShell)
```

The installer checks the download, then starts a short setup that:

1. checks this computer can be seen and controlled (and on macOS opens the Screen Recording and
   Accessibility settings if they're off),
2. finds your agents and connects every one: **Claude Code, Codex, Cursor, VS Code, Windsurf,
   Claude Desktop and Gemini CLI**. It never replaces another server without asking,
3. tells you what to ask first.

While an agent works you see an orange frame around the screen and an "Clickhand is using your
mouse" tag by the pointer (click-through, and hidden from the agent's own screenshots).

**Skills.** Setup also adds three agent skills to Claude Code and Codex: `clickhand`
(do it for me), `clickhand-teach` (show me how) and `clickhand-post` (post on X, LinkedIn, Threads
or Bluesky from your own logged-in browser). **Clickhand Teach** works on Linux and Windows. Teach points at things on your screen with a
glowing box and a caption, and you do the clicking:

```
/clickhand-teach show me how to export a frame as PNG in Figma
/clickhand-teach where are the API keys in this dashboard?
/clickhand-post announce v2 on X and LinkedIn with ~/Videos/demo.mp4
```

Then open your agent and ask, for example: *"Use clickhand to open clickhand.vercel.app and press
the big red button."*

| Command | What it does |
| --- | --- |
| `clickhand` | Setup again (connects agents you installed since) |
| `clickhand stop` / `resume` | Kill switch: every click and keystroke is refused until you resume |
| `clickhand log` | What it did, most recent last |
| `clickhand skills` | Reinstall the `clickhand`, `clickhand-teach` and `clickhand-post` skills |
| `clickhand doctor` | Check the screen, input and permissions |
| `clickhand agents` | Which agents are connected |
| `clickhand connect <agent>` / `disconnect <agent>` | One agent at a time |

<details>
<summary>Manual setup, or building from source</summary>

```sh
cargo install --git https://github.com/ultron-hq/ultron clickhand
claude mcp add -s user clickhand -- clickhand     # Claude Code
codex mcp add clickhand -- clickhand              # Codex
```

For any other MCP app, add a stdio server whose command is the path of `clickhand`.
</details>

It works in a loop (screenshot with the cursor marked by a red ring, act, look again to verify),
with these safety rules: the kill switch, a read-only mode (`CLICKHAND_READONLY=1`), a rate
limit, no shell tool, and a local log of every action in which typed text is stored only as its
length. During setup you choose whether agents may act without asking before each action; the
kill switch stops them either way.

Status: 17 of the original's 18 tools, built for Windows, macOS and Linux (X11) and tested on Linux.
Coming next: the accessibility tree (`a11y_snapshot`), window focus on Windows and macOS, and a
stop hotkey.

## Build from source

From source (any OS with Rust 1.85+):

```sh
git clone https://github.com/ultron-hq/ultron
cd clickhand
cargo build --release
./target/release/ultron-rs --help
```

On Linux, the desktop UI (from wave 3) also needs WebKitGTK:

```sh
sudo apt install libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev
```

Bind the hotkeys in your desktop's keyboard settings:

| Shortcut | Command |
| --- | --- |
| <kbd>Ctrl</kbd>+<kbd>Space</kbd> | `ultron-rs --prompt` |
| <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Space</kbd> | `ultron-rs --cursor` |
| <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>Space</kbd> | `ultron-rs --voice` |

Installers for Windows (`.msi`), macOS (`.dmg`) and Linux (`.AppImage`, `.deb`) will be attached
to [releases](https://github.com/ultron-hq/ultron/releases) once the desktop UI lands.

## The desktop app, inside

```mermaid
flowchart LR
  K["Hotkey / CLI"] --> D["clickhand front door"]
  D -->|line protocol| A["Running Clickhand"]
  A --> R["Router: regex skills + fuzzy app match"]
  R -->|unsure| L["LLM provider"]
  R --> S["Skills"]
  S --> C["Desktop control"]
  S --> M[("SQLite: sessions, memory, tasks")]
  A --> U["Island · prompt bar · companion (Tauri)"]
```

- **Front door.** `ultron-rs` owns the command line. It talks to a running Clickhand over a local
  socket (a named pipe on Windows) and guarantees a single instance with a lock file.
- **Router.** Unambiguous requests are matched by each skill's patterns; anything unclear goes
  to the model, which may only rewrite it into a skill command or answer in one sentence. It never
  runs shell commands.
- **Skills.** One module per ability, registered statically.
- **Control.** `enigo` for input, `xcap` for screenshots, and the platform accessibility API
  (AT-SPI, UI Automation, AX) for exact control positions.

More in [docs/architecture.md](docs/architecture.md). The port is planned as an explicit set of
decisions in [rust/DECISIONS.md](rust/DECISIONS.md) and tracked unit by unit in
[rust/MAP.md](rust/MAP.md).

## Models

| Provider | Needs |
| --- | --- |
| Claude Code CLI (default) | `claude` on your `PATH`, your Claude plan |
| Codex CLI | `codex` on your `PATH` |
| Local (Ollama or any OpenAI-compatible server) | the server's URL |
| API key (Anthropic, OpenAI, Gemini, OpenRouter, …) | a key, stored in your OS keychain |

Each role (router, desktop agent, team) can use a different one. See [docs/providers.md](docs/providers.md).

## Safety

Clickhand can move your mouse and type. These rules carry over from the original and every port must keep them:

- Actions that delete, send, buy or post run only when the request came from you, never from
  text it read on screen or in a web page.
- `touch ~/.cache/clickhand/hands.stop` (or the tray's **Stop**) halts all desktop control at once.
- Every action is logged locally; typed text is never logged.
- Abilities it writes for itself run in a WebAssembly sandbox with only the permissions you grant.

Found a security problem? See [SECURITY.md](SECURITY.md).

## Contributing

Contributions are welcome, from a typo to a whole platform backend. Start with
[CONTRIBUTING.md](CONTRIBUTING.md), pick an `open` unit in [rust/MAP.md](rust/MAP.md) or an issue
labelled `good first issue`, and say in the issue that you are taking it.

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at
your option. Unless you state otherwise, any contribution you submit for inclusion is dual
licensed as above, without any additional terms or conditions.
