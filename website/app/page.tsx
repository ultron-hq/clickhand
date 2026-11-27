import { Keyboard, Timer, BookMarked, Cpu, Mic, Puzzle, Scale, Monitor, Plug } from "lucide-react";
import { MaskField } from "@/components/MaskField";
import { Header } from "@/components/Header";
import { Workstation } from "@/components/Workstation";
import { LoopDiagram } from "@/components/LoopDiagram";
import { LiveTimer } from "@/components/LiveTimer";
import { KillSwitch } from "@/components/KillSwitch";
import { InstallBox } from "@/components/InstallBox";
import { HeroClip } from "@/components/HeroClip";
import { Footer } from "@/components/Footer";
import { DiscordIcon } from "@/components/DiscordIcon";
import { REPO, getStars, DISCORD } from "@/lib/site";


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
  { icon: Keyboard, title: "Prompt bar", body: <>Opens in the middle of the screen on <Keys keys={["Ctrl", "Space"]} />, or click the clock</> },
  { icon: Timer, title: "Timers and focus time", body: "Start a timer or stopwatch by asking. The clock counts it down and every session is logged" },
  { icon: Cpu, title: "Your AI does the rest", body: "Anything else goes to Claude Code or Codex, which can use the computer through OpenHUD" },
  { icon: BookMarked, title: "Memory", body: "Tell it a fact or a phrase once. It keeps the plans that worked and takes corrections like “no, I meant…”", soon: true },
  { icon: Mic, title: "Voice", body: "Push to talk with Whisper on your machine, or a live conversation with Gemini", soon: true },
  { icon: Puzzle, title: "Abilities it writes", body: "Ask for a tool it lacks and it builds one, sandboxed in WebAssembly with only the permissions you grant", soon: true },
];

const SKILLS = [
  {
    name: "teach", tag: "/openhud-teach", title: "Show me how",
    body: "It points at each button on your screen with a glowing box and a caption, waits for you to click, then shows the next step. It never clicks for you",
    cmd: "/openhud-teach how do I add RSI on TradingView?",
    video: "/media/teach.mp4", poster: "/media/teach.jpg", alt: "Codex teaching how to add the RSI indicator on TradingView with on-screen highlights",
  },
  {
    name: "post", tag: "/openhud-post", title: "Post it for me",
    body: "It writes the post, attaches your video, publishes from your own logged-in browser on X or LinkedIn, then checks it went up. No API keys",
    cmd: "/openhud-post announce v2 on X with ~/demo.mp4",
    video: "/media/post.mp4", poster: "/media/post.jpg", alt: "Codex writing and publishing a post on X by itself",
  },
  {
    name: "hands", tag: "/openhud", title: "Do it for me",
    body: "Anything that needs a person at the computer: forms, settings, apps without an API. It looks, acts, and checks every step",
    cmd: "/openhud rename these 40 files in Finder",
    video: "/media/first-user.mp4", poster: "/media/first-user.jpg", alt: "A first user cloning OpenHUD and Codex pressing a button on the website by itself",
  },
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
  ["Ollama, LM Studio, llama.cpp", "Runs on your machine", "Coming"],
  ["Anthropic, OpenAI, Gemini, OpenRouter", "An API key in your OS keychain", "Coming"],
];

export const revalidate = 3600;

export default async function Home() {
  const stars = await getStars();
  return (
    <>
      <Header repo={REPO} stars={stars} />

      <main>
        <section className="hero">
          <MaskField />
          <div className="hero-inner">
            <div className="hero-copy">
              <h1>Let any AI agent use your PC like a human</h1>
              <p className="lede">
                OpenHUD is an open-source desktop agent. Press <Keys keys={["Ctrl", "Space"]} /> and ask, or give Claude Code, Codex and any MCP agent the mouse and keyboard
              </p>
              <InstallBox />
              <div className="actions">
                <a className="btn btn-quiet" href="#demo">See it work</a>
                <a className="btn btn-quiet" href={REPO}>View on GitHub</a>
                <a className="btn btn-quiet" href={DISCORD}><DiscordIcon size={16} />Discord</a>
              </div>
              <ul className="facts">
                <li><Scale size={16} strokeWidth={1.5} aria-hidden="true" />Open source, MIT or Apache-2.0</li>
                <li><Monitor size={16} strokeWidth={1.5} aria-hidden="true" />Windows, macOS and Linux</li>
                <li><Plug size={16} strokeWidth={1.5} aria-hidden="true" />Works with Claude Code, Codex and MCP</li>
              </ul>
            </div>
          </div>
        </section>

        <HeroClip />

        <section className="demo" id="demo">
          <div className="section-head">
            <h2>Your agent asks, OpenHUD clicks</h2>
            <p>A Claude Code session on the left, the desktop it controls on the right. Every step is a screenshot, an action and a check</p>
            <p className="note">Hands ships today for Windows, macOS and Linux. The full desktop app is being rewritten in Rust, <a href={`${REPO}/blob/main/ROADMAP.md`}>see the roadmap</a></p>
          </div>
          <Workstation />
        </section>

        <section className="hands" id="hands">
          <div className="hands-copy">
            <h2>Hands for your agent</h2>
            <p>
              Coding agents read and write files, but they can’t click. OpenHUD is an <a href="https://modelcontextprotocol.io">MCP</a> server that lets them see the screen, move the mouse, click, scroll and type, so anything that needs a person at the computer can be handed off
            </p>
            <pre><code><span className="c"># 1. install, then answer the setup</span>{"\n"}curl -fsSL https://openhud.vercel.app/install.sh | sh{"\n"}<span className="c"># 2. ask your agent</span>{"\n"}"Use openhud to book the 7pm table on OpenTable"{"\n"}<span className="c"># stop it any time</span>{"\n"}openhud stop</code></pre>
            <p className="note">One Rust binary for Windows, macOS and Linux. Setup finds Claude Code, Codex, Cursor, VS Code, Windsurf, Claude Desktop and Gemini CLI and connects each one</p>
          </div>
          <LoopDiagram />
          <ol className="steps">
            <li><span className="step-n">01</span><b>Look</b><p>A screenshot of the screen, with the cursor marked by a red ring</p></li>
            <li><span className="step-n">02</span><b>Act</b><p>A click, keystroke or scroll, at exact control positions from the accessibility tree when the app exposes them</p></li>
            <li><span className="step-n">03</span><b>Check</b><p>Another look. It moves on only when the screen shows the step worked</p></li>
          </ol>
        </section>

        <section className="skills" id="skills">
          <div className="section-head">
            <h2>Three skills, one sentence each</h2>
            <p>Setup adds them to Claude Code and Codex. Type the slash command, say what you want, and watch it happen on your own screen</p>
          </div>
          <div className="skill-grid">
            {SKILLS.map((k) => (
              <article key={k.name} className="skill">
                <video className="skill-video" src={k.video} poster={k.poster} controls muted playsInline preload="none" aria-label={k.alt} />
                <div className="skill-body">
                  <p className="skill-tag">{k.tag}</p>
                  <h3>{k.title}</h3>
                  <p>{k.body}</p>
                  <code className="skill-cmd">{k.cmd}</code>
                </div>
              </article>
            ))}
          </div>
          <p className="note">Recorded on a real desktop, sped up to fit. <a href="/docs#skills">How the skills work</a></p>
        </section>

        <section className="app" id="app">
          <div className="app-visual" aria-hidden="true">
            <LiveTimer />
            <div className="promptbar">
              <img src="/logo.svg" alt="" width={20} height={20} />
              <span>start a 25 minute timer</span>
              <Keys keys={["Enter"]} />
            </div>
          </div>
          <div>
            <h2>OpenHUD, the app</h2>
            <p className="intro">A flip clock that lives at the top of your screen and answers when you talk to it</p>
            <ul className="rows">
              {APP.map(({ icon: Icon, title, body, soon }) => (
                <li key={title}>
                  <Icon size={18} strokeWidth={1.5} aria-hidden="true" />
                  <div><b>{title}{soon && <em className="soon">Coming</em>}</b><p>{body}</p></div>
                </li>
              ))}
            </ul>
          </div>
        </section>

        <section className="models">
          <div className="models-head">
            <h2>Bring the model you already pay for</h2>
            <p className="intro">OpenHUD works with the AI apps you already have. Claude Code and Codex today, local models and API keys next</p>
          </div>
          <ul className="providers">
            {MODELS.map(([p, n, tag]) => (
              <li key={p} className={tag === "Default" ? "provider provider-default" : "provider"}>
                <b>{p}</b>
                <span>{n}</span>
                {tag && <em>{tag}</em>}
              </li>
            ))}
          </ul>
        </section>

        <section className="safety" id="safety">
          <KillSwitch />
          <div>
            <h2>You stay in charge</h2>
            <ul>
              {SAFETY.map((s) => (
                <li key={s.title}><b>{s.title}</b><p>{s.body}</p></li>
              ))}
            </ul>
            <pre className="stop-cmd"><code><span className="c"># the real kill switch</span>{"\n"}openhud stop{"\n"}<span className="c"># and what it did</span>{"\n"}openhud log</code></pre>
          </div>
        </section>

        <section className="build">
          <div className="build-inner">
            <h2>Build it with us</h2>
            <p>MIT or Apache-2.0, written in Rust with a Tauri interface. Every piece of the rewrite is checked against the original app before it ships, and the open pieces are listed as issues</p>
            <pre><code>git clone {REPO}{"\n"}cd openhud && cargo build --release{"\n"}./target/release/openhud doctor</code></pre>
            <div className="actions">
              <a className="btn btn-primary" href={`${REPO}/issues`}>Pick an issue</a>
              <a className="btn btn-quiet" href={`${REPO}/blob/main/CONTRIBUTING.md`}>Contributing guide</a>
              <a className="btn btn-quiet" href={DISCORD}><DiscordIcon size={16} />Join the Discord</a>
            </div>
            <p className="build-note">On Discord: what's coming next, early builds, and help setting it up</p>
          </div>
        </section>
      </main>

      <Footer />
    </>
  );
}
