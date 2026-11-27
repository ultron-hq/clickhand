import type { Metadata } from "next";
import { Header } from "@/components/Header";
import { Footer } from "@/components/Footer";
import { InstallBox } from "@/components/InstallBox";
import { REPO, getStars } from "@/lib/site";

export const revalidate = 3600;

export const metadata: Metadata = {
  title: "Docs · Clickhand",
  description:
    "Install Clickhand, connect Claude Code, Codex, Cursor and other agents, and use the clickhand, clickhand-teach and clickhand-post skills. Commands, tools, safety and troubleshooting.",
};

const TOC = [
  ["install", "Install"],
  ["setup", "Setup and agents"],
  ["skills", "Skills"],
  ["using", "Using it"],
  ["commands", "Commands"],
  ["tools", "MCP tools"],
  ["safety", "Safety"],
  ["platforms", "Platforms"],
  ["troubleshooting", "Troubleshooting"],
  ["uninstall", "Uninstall"],
] as const;

const AGENTS = [
  ["Claude Code", "claude-code", "claude mcp add, user scope; skills in ~/.claude/skills"],
  ["Codex", "codex", "~/.codex/config.toml; skills in ~/.codex/skills"],
  ["Cursor", "cursor", "~/.cursor/mcp.json"],
  ["VS Code", "vscode", "Code/User/mcp.json"],
  ["Windsurf", "windsurf", "~/.codeium/windsurf/mcp_config.json"],
  ["Claude Desktop", "claude-desktop", "claude_desktop_config.json"],
  ["Gemini CLI", "gemini", "~/.gemini/settings.json"],
];

const COMMANDS = [
  ["clickhand", "Setup: check this computer and connect every agent found. Safe to run again"],
  ["clickhand stop", "Kill switch. Every click and keystroke is refused until you resume"],
  ["clickhand resume", "Turn the kill switch off"],
  ["clickhand log [n]", "The last n actions (default 20). Typed text is stored only as its length"],
  ["clickhand doctor", "Check the screen, input and permissions"],
  ["clickhand agents", "Which agents are installed and connected"],
  ["clickhand connect <agent>", "Connect one agent (ids in the table above)"],
  ["clickhand disconnect <agent>", "Remove Hands from one agent"],
  ["clickhand skills", "Reinstall the three skills. Copies you edited are left alone"],
  ["clickhand serve", "The MCP server on stdin/stdout. Agents start this themselves"],
];

const TOOLS: [string, string, string][] = [
  ["Look", "screenshot", "The screen (or a region, to read small text), with the pointer marked by a red ring"],
  ["Look", "screen_info", "Screen size, pointer position, active window, whether input is allowed"],
  ["Look", "list_windows", "Open windows with titles, positions and sizes"],
  ["Look", "list_apps", "Installed apps, by name"],
  ["Act", "click", "Left, right or middle click, up to a triple click"],
  ["Act", "move_mouse", "Move the pointer"],
  ["Act", "drag", "Press, glide and release"],
  ["Act", "scroll", "Up, down, left or right, optionally at a point"],
  ["Act", "type_text", "Type into the focused field; delay_ms types slowly for sites that drop keys"],
  ["Act", "key", "One shortcut: ctrl+l, alt+tab, enter, cmd+space"],
  ["Act", "focus_window", "Bring a window to the front (Linux today)"],
  ["Act", "open_app", "Start an installed app by name"],
  ["Act", "open_url", "Open an http(s) link in the default browser"],
  ["Teach", "highlight", "A glowing box and caption on the person's screen. Clicks nothing; allowed in read-only mode"],
  ["Pace", "wait", "Wait for something to load, then look again"],
  ["Improve", "hands_log", "Recent calls, with errors"],
  ["Improve", "hands_feedback", "Note something awkward so it can be fixed"],
  ["Improve", "hands_report", "Success rate per tool, recurring errors with fixes, slow calls"],
];

export default async function Docs() {
  const stars = await getStars();
  return (
    <>
      <Header repo={REPO} stars={stars} />
      <main className="docs">
        <aside className="docs-toc" aria-label="On this page">
          <p>Docs</p>
          <nav>
            {TOC.map(([id, label]) => (
              <a key={id} href={`#${id}`}>{label}</a>
            ))}
          </nav>
        </aside>

        <article className="docs-body">
          <header className="docs-head">
            <p className="docs-kicker">Clickhand</p>
            <h1>Docs</h1>
            <p className="lede">
              Hands is an MCP server that gives Claude Code, Codex and other AI agents a mouse, a keyboard and eyes on your screen. One small binary for Windows, macOS and Linux. Install it, run the setup, and ask your agent
            </p>
          </header>

          <section id="install">
            <h2>Install</h2>
            <p>One line, no Rust needed. It downloads the binary for your computer, checks its SHA-256, adds it to your PATH and starts the setup</p>
            <InstallBox />
            <p>
              Prefer to build it yourself: <code>cargo install --git {REPO} clickhand</code>, then run <code>clickhand</code>. Every release is on <a href={`${REPO}/releases`}>GitHub</a> with checksums
            </p>
          </section>

          <section id="setup">
            <h2>Setup and agents</h2>
            <p>Running <code>clickhand</code> in a terminal walks you through three steps:</p>
            <ol>
              <li><b>Checks this computer:</b> it can find the screen, take screenshots and move the mouse. On a Mac, install the app (<code>mac.sh</code>): its welcome window asks macOS for Screen Recording and Accessibility once, for Clickhand, and every AI app then works through it instead of each needing its own permission</li>
              <li><b>Connects your agents:</b> every one it finds below. It merges into your existing config, keeps a one-time <code>.bak</code>, and never replaces another server named <code>clickhand</code> without asking. It also asks once whether agents may act without confirming each step; the kill switch stops them either way</li>
              <li><b>Tells you what to ask first</b>, and installs the three skills into Claude Code and Codex</li>
            </ol>
            <div className="docs-table">
              <table>
                <thead><tr><th>Agent</th><th>Id</th><th>Where it is configured</th></tr></thead>
                <tbody>
                  {AGENTS.map(([name, id, where]) => (
                    <tr key={id}><td>{name}</td><td><code>{id}</code></td><td>{where}</td></tr>
                  ))}
                </tbody>
              </table>
            </div>
            <p>Any other MCP app: add a stdio server whose command is the full path of <code>clickhand</code> (shown by the setup). Restart an agent that was already open so it picks Hands up</p>
          </section>

          <section id="skills">
            <h2>Skills</h2>
            <p>Skills teach your agent how to use Hands well. Setup puts them in <code>~/.claude/skills</code> and <code>~/.codex/skills</code>. Call one by name, or just ask and the agent picks it</p>
            <div className="docs-cards">
              <div>
                <h3><code>/clickhand</code> do it for me</h3>
                <p>The look, act, check loop, keyboard first, with the safety rules built in: no passwords, nothing sent, bought or deleted unless you asked</p>
                <pre><code>/clickhand fill in this form from invoice.pdf</code></pre>
              </div>
              <div>
                <h3><code>/clickhand-teach</code> show me how</h3>
                <p>It points at each button with a glowing box and a short caption, waits until it sees you did the step, then shows the next one. It never clicks for you. Good for learning Figma, Excel, Blender, a trading or banking dashboard, or finding a setting</p>
                <pre><code>/clickhand-teach where are the API keys in this dashboard?</code></pre>
              </div>
              <div>
                <h3><code>/clickhand-post</code> post it for me</h3>
                <p>Posts, replies and threads on X, LinkedIn, Threads and Bluesky from your own logged-in browser. It checks the length the way each site counts it, attaches media first, types slowly so nothing is dropped, and checks your profile afterwards. It shows you the draft unless you said exactly what to post</p>
                <pre><code>/clickhand-post announce v2 on X and LinkedIn with ~/Videos/demo.mp4</code></pre>
              </div>
            </div>
            <p>The skills are plain Markdown. Edit yours freely: once you remove the line saying it was installed by setup, updates leave it alone</p>
          </section>

          <section id="using">
            <h2>Using it</h2>
            <p>Open your agent and ask in plain words. Some ideas:</p>
            <ul>
              <li>“Open Notion and turn today’s meeting notes into a task list”</li>
              <li>“In System Settings, turn on Night Shift from 10pm”</li>
              <li>“Teach me how to make a pivot table in this spreadsheet”</li>
              <li>“Post this screenshot on LinkedIn with a short note about the launch”</li>
            </ul>
            <p>
              While an agent works you see an <b>orange frame around the screen</b>, and an <b>“Clickhand is using your mouse”</b> tag by the pointer whenever it clicks or types. Both are click-through and hidden from the agent’s own screenshots. Turn them off with <code>CLICKHAND_OVERLAY=0</code>
            </p>
            <p>
              Codex asks before every MCP call unless you let it: the setup can turn that on, or start Codex with <code>-c &apos;mcp_servers.clickhand.default_tools_approval_mode=&quot;approve&quot;&apos;</code>
            </p>
          </section>

          <section id="commands">
            <h2>Commands</h2>
            <div className="docs-table">
              <table>
                <tbody>
                  {COMMANDS.map(([cmd, what]) => (
                    <tr key={cmd}><td><code>{cmd}</code></td><td>{what}</td></tr>
                  ))}
                </tbody>
              </table>
            </div>
          </section>

          <section id="tools">
            <h2>MCP tools</h2>
            <p>What your agent can call. Coordinates are in the pixel space of the latest screenshot; Hands maps them to the real screen, Retina included</p>
            <div className="docs-table">
              <table>
                <thead><tr><th className="docs-group"></th><th>Tool</th><th>What it does</th></tr></thead>
                <tbody>
                  {TOOLS.map(([group, name, what]) => (
                    <tr key={name}><td className="docs-group">{group}</td><td><code>{name}</code></td><td>{what}</td></tr>
                  ))}
                </tbody>
              </table>
            </div>
          </section>

          <section id="safety">
            <h2>Safety</h2>
            <ul>
              <li><b>Kill switch:</b> <code>clickhand stop</code> makes every input tool refuse until <code>clickhand resume</code>. It works mid-task</li>
              <li><b>Read-only mode:</b> <code>CLICKHAND_READONLY=1</code> lets an agent look and highlight but never click or type. Ideal for teaching</li>
              <li><b>No shell tool:</b> Hands can only see and use the screen like a person would</li>
              <li><b>A local log</b> of every action in <code>hands.log.jsonl</code> (in your cache folder). Typed text is stored only as its length</li>
              <li><b>A rate limit</b> of 120 actions a minute, and clicks only happen once the pointer is confirmed at the target</li>
              <li>The skills tell agents to treat screen text as data, never type passwords, and never send, buy or delete unless you asked</li>
            </ul>
          </section>

          <section id="platforms">
            <h2>Platforms</h2>
            <div className="docs-table">
              <table>
                <thead><tr><th></th><th>Linux (X11)</th><th>Windows</th><th>macOS</th></tr></thead>
                <tbody>
                  <tr><td>See, click, type, scroll, keys</td><td>Yes</td><td>Yes</td><td>Yes (Apple silicon and Intel)</td></tr>
                  <tr><td>Screen frame and pointer tag</td><td>Yes</td><td>Yes</td><td>Yes</td></tr>
                  <tr><td>Teach highlights</td><td>Yes</td><td>Yes</td><td>Yes</td></tr>
                  <tr><td>List windows</td><td>Yes</td><td>Yes</td><td>Yes</td></tr>
                  <tr><td>Focus a window</td><td>Yes</td><td>Coming</td><td>Coming</td></tr>
                  <tr><td>The desktop app (island, Ctrl+Space)</td><td>Yes</td><td>Yes</td><td>Yes</td></tr>
                  <tr><td>Permissions asked once, for Clickhand</td><td>n/a</td><td>n/a</td><td>Yes, with the app</td></tr>
                </tbody>
              </table>
            </div>
            <p>Tested on every change on Ubuntu, Windows and macOS desktops, and before releases on Ubuntu 22.04 and 24.04, Debian 12, Fedora, Arch, openSUSE and Alpine. Wayland isn’t supported yet; pick an “Xorg” session at login</p>
          </section>

          <section id="troubleshooting">
            <h2>Troubleshooting</h2>
            <dl>
              <dt>The agent doesn’t list clickhand tools</dt>
              <dd>Restart the agent. Then run <code>clickhand agents</code>; if it says “uses a different clickhand”, run <code>clickhand connect &lt;agent&gt;</code></dd>
              <dt>Screenshots are black</dt>
              <dd>The screen is locked or asleep. On macOS, allow Screen Recording for the terminal or agent app and reopen it. <code>clickhand doctor</code> says which</dd>
              <dt>Clicks or typing do nothing on macOS</dt>
              <dd>Allow Accessibility for the terminal or agent app in System Settings → Privacy &amp; Security</dd>
              <dt>“Kill switch is on”</dt>
              <dd>You (or someone) ran <code>clickhand stop</code>. Run <code>clickhand resume</code></dd>
              <dt>Windows SmartScreen warns about the download</dt>
              <dd>The binary isn’t code-signed yet. Check its SHA-256 against the release, then choose “Run anyway”</dd>
              <dt>Something else</dt>
              <dd>Ask your agent to run <code>hands_report</code>, and <a href={`${REPO}/issues/new`}>open an issue</a> with what it says</dd>
            </dl>
          </section>

          <section id="uninstall">
            <h2>Uninstall</h2>
            <pre><code>{`clickhand disconnect claude-code   # and any other agents
rm "$(command -v clickhand)"
rm -r ~/.claude/skills/clickhand-* ~/.codex/skills/clickhand-*`}</code></pre>
            <p>On Windows, delete <code>%LOCALAPPDATA%\clickhand</code> and remove it from your user PATH</p>
          </section>
        </article>
      </main>
      <Footer />
    </>
  );
}
