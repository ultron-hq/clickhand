import { Keyboard, Timer, BookMarked, Cpu, Mic, Puzzle, Scale, Monitor, Plug } from "lucide-react";
import { MaskField } from "@/components/MaskField";
import { Workstation } from "@/components/Workstation";
import { SplitFlap } from "@/components/SplitFlap";

const REPO = "https://github.com/ultron-hq/ultron";

function Keys({ keys }: { keys: string[] }) {
  return (
    <span className="keys">
      {keys.map((k) => (
        <kbd key={k}>{k}</kbd>
      ))}
    </span>
  );
}

const APP = [
  { icon: Keyboard, title: "Prompt bar", body: <>Opens in the middle of the screen on <Keys keys={["Ctrl", "Space"]} />, or beside your cursor with Shift</> },
  { icon: Timer, title: "Timers and focus time", body: "Start a timer by asking. Every session is logged, so you can ask how much you worked this week" },
  { icon: BookMarked, title: "Memory", body: "Tell it a fact or a phrase once. It keeps the plans that worked and takes corrections like “no, I meant…”" },
  { icon: Cpu, title: "Your choice of model", body: "Claude Code, Codex, a local model through Ollama, or an API key from any provider, one per job" },
  { icon: Mic, title: "Voice", body: "Push to talk with Whisper on your machine, or a live conversation with Gemini" },
  { icon: Puzzle, title: "Abilities it writes", body: "Ask for a tool it lacks and it builds one, sandboxed in WebAssembly with only the permissions you grant" },
];

const SAFETY = [
  { title: "Your requests only", body: "It acts on what you asked, never on instructions it reads on a screen or a web page" },
  { title: "One command stops it", body: "A kill switch halts all desktop control at once" },
  { title: "No passwords", body: "It never types them, and what it types is never logged" },
  { title: "A record of every action", body: "Each click and keystroke it makes is logged on your machine" },
];

const MODELS = [
  ["Claude Code CLI", "Your Claude plan, no key", "Default"],
  ["Codex CLI", "Your ChatGPT plan", ""],
  ["Ollama, LM Studio, llama.cpp", "Runs on your machine", "Offline"],
  ["Anthropic, OpenAI, Gemini, OpenRouter", "An API key in your OS keychain", ""],
];

export default function Home() {
  return (
    <>
      <header className="nav">
        <a className="brand" href="/">
          <img src="/logo.svg" alt="" width={24} height={24} />
          Ultron
        </a>
        <nav aria-label="Main">
          <a href="#hands">Hands</a>
          <a href="#app">The app</a>
          <a href="#safety">Safety</a>
          <a href={`${REPO}/blob/main/ROADMAP.md`}>Roadmap</a>
        </nav>
        <a className="btn btn-quiet btn-sm" href={REPO}>
          GitHub
        </a>
      </header>

      <main>
        <section className="hero">
          <MaskField />
          <div className="hero-inner">
            <div className="hero-copy">
              <h1>Let any AI agent use your PC like a human</h1>
              <p className="lede">
                Ultron is an open-source desktop agent. Press <Keys keys={["Ctrl", "Space"]} /> and ask, or give Claude Code, Codex and any MCP agent the mouse and keyboard
              </p>
              <div className="actions">
                <a className="btn btn-primary" href={REPO}>View on GitHub</a>
                <a className="btn btn-quiet" href="#demo">See it work</a>
              </div>
              <ul className="facts">
                <li><Scale size={16} strokeWidth={1.5} aria-hidden="true" />Open source, MIT or Apache-2.0</li>
                <li><Monitor size={16} strokeWidth={1.5} aria-hidden="true" />Windows, macOS and Linux</li>
                <li><Plug size={16} strokeWidth={1.5} aria-hidden="true" />Works with Claude Code, Codex and MCP</li>
              </ul>
            </div>
          </div>
        </section>

        <section className="demo" id="demo">
          <div className="section-head">
            <h2>Your agent asks, Ultron clicks</h2>
            <p>A Claude Code session on the left, the desktop it controls on the right. Every step is a screenshot, an action and a check</p>
            <p className="note">Early development. The Rust rewrite is under way, <a href={`${REPO}/blob/main/ROADMAP.md`}>see what works today</a></p>
          </div>
          <Workstation />
        </section>

        <section className="hands" id="hands">
          <div>
            <h2>Hands for your agent</h2>
            <p>
              Coding agents read and write files, but they can’t click. Ultron Hands is an <a href="https://modelcontextprotocol.io">MCP</a> server that lets them see the screen, move the mouse, click, scroll and type, so anything that needs a person at the computer can be handed off
            </p>
            <pre><code><span className="c"># Claude Code</span>{"\n"}claude mcp add ultron-hands -- ultron-hands{"\n"}<span className="c"># Codex</span>{"\n"}codex mcp add ultron-hands -- ultron-hands</code></pre>
            <p className="note">Hands runs in the original Ultron today and arrives in the Rust app with <a href={`${REPO}/blob/main/ROADMAP.md`}>wave 4</a></p>
          </div>
          <ol className="steps">
            <li><span className="step-n">1</span><div><b>Look</b><p>A screenshot of the screen, with the cursor marked by a red ring</p></div></li>
            <li><span className="step-n">2</span><div><b>Act</b><p>A click, keystroke or scroll, at exact control positions from the accessibility tree when the app exposes them</p></div></li>
            <li><span className="step-n">3</span><div><b>Check</b><p>Another look. It moves on only when the screen shows the step worked</p></div></li>
          </ol>
        </section>

        <section className="app" id="app">
          <div className="app-visual" aria-hidden="true">
            <div className="island island-still">
              <SplitFlap text="25:00" label="Timer at 25 minutes" />
              <span className="island-say">Timer started, 25 minutes</span>
            </div>
            <div className="promptbar">
              <img src="/logo.svg" alt="" width={20} height={20} />
              <span>start a 25 minute timer</span>
              <Keys keys={["Enter"]} />
            </div>
          </div>
          <div>
            <h2>Ultron, the app</h2>
            <p className="intro">A flip clock that lives at the top of your screen and answers when you talk to it</p>
            <ul className="rows">
              {APP.map(({ icon: Icon, title, body }) => (
                <li key={title}>
                  <Icon size={18} strokeWidth={1.5} aria-hidden="true" />
                  <div><b>{title}</b><p>{body}</p></div>
                </li>
              ))}
            </ul>
          </div>
        </section>

        <section className="models">
          <h2>Bring the model you already pay for</h2>
          <table>
            <thead><tr><th scope="col">Provider</th><th scope="col">What you need</th><th scope="col"><span className="sr">Note</span></th></tr></thead>
            <tbody>
              {MODELS.map(([p, n, tag]) => (
                <tr key={p}><td>{p}</td><td>{n}</td><td className="muted">{tag}</td></tr>
              ))}
            </tbody>
          </table>
        </section>

        <section className="safety" id="safety">
          <h2>You stay in charge</h2>
          <ul>
            {SAFETY.map((s) => (
              <li key={s.title}><b>{s.title}</b><p>{s.body}</p></li>
            ))}
          </ul>
        </section>

        <section className="build">
          <h2>Build it with us</h2>
          <p>MIT or Apache-2.0, written in Rust with a Tauri interface. Every piece of the rewrite is checked against the original app before it ships, and the open pieces are listed as issues</p>
          <pre><code>git clone {REPO}{"\n"}cd ultron && cargo build --release{"\n"}./target/release/ultron-rs --help</code></pre>
          <div className="actions">
            <a className="btn btn-primary" href={`${REPO}/issues`}>Pick an issue</a>
            <a className="btn btn-quiet" href={`${REPO}/blob/main/CONTRIBUTING.md`}>Contributing guide</a>
          </div>
        </section>
      </main>

      <footer className="foot">
        <span className="brand brand-sm"><img src="/logo.svg" alt="" width={18} height={18} />Ultron</span>
        <nav aria-label="Footer">
          <a href={REPO}>GitHub</a>
          <a href={`${REPO}/discussions`}>Discussions</a>
          <a href={`${REPO}/blob/main/SECURITY.md`}>Security</a>
        </nav>
      </footer>
    </>
  );
}
