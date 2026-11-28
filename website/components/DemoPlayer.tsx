"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Pause, Play, RotateCcw } from "lucide-react";
import { stopped } from "./stop";

/**
 * The demo: a desktop driven the way OpenHUD drives one, one scenario per skill.
 * Left, the agent's console with its real tool calls; right, the app it uses, under the
 * HUD: a scan on every screenshot, a reticle that glides to each target, brackets that
 * lock on with coordinates, a ripple on each click, ✓ stamps on checks. In Teach the
 * agent never clicks: it highlights, and a separate white "person" cursor clicks.
 * Plays while on screen; tabs, play/pause and replay below. Pauses with the page's
 * kill switch.
 */

type Id = "do" | "teach" | "post";
type Scn = { id: Id; skill: string; label: string; prompt: string; marks: string[] };

const SCENARIOS: Scn[] = [
  { id: "do", skill: "/openhud", label: "Fill a form from a PDF", prompt: "fill the expense form from invoice-nov.pdf",
    marks: ["Read", "Look", "Vendor", "Amount", "Date", "Submit", "Check"] },
  { id: "teach", skill: "/openhud-teach", label: "Teach me a new app", prompt: "how do I add RSI to this chart?",
    marks: ["Look", "Indicators", "Search", "Pick RSI", "Check"] },
  { id: "post", skill: "/openhud-post", label: "Post a launch on X", prompt: "announce v0.3 on X with ~/Videos/demo.mp4",
    marks: ["Open X", "Write", "Attach", "Post", "Check"] },
];

type Line = { kind: "user" | "tool" | "result" | "say"; text: string; ms?: number };
type Rect = { x: number; y: number; w: number; h: number };

const POST = "OpenHUD v0.3 is out: any AI agent can now see your screen, click and type, and stop the moment you say so. Open source.";

const pad4 = (n: number) => String(Math.max(0, Math.round(n))).padStart(4, "0");

// A deterministic little market for the chart.
const CANDLES = (() => {
  let p = 182;
  return Array.from({ length: 46 }, (_, i) => {
    const o = p;
    const c = o + Math.sin(i * 0.55) * 2.2 + Math.cos(i * 1.7) * 1.4 + (i > 30 ? 0.9 : 0.1);
    const hi = Math.max(o, c) + 1 + Math.abs(Math.sin(i * 2.3)) * 1.5;
    const lo = Math.min(o, c) - 1 - Math.abs(Math.cos(i * 1.9)) * 1.5;
    p = c;
    return { o, c, hi, lo };
  });
})();

function rsi(period = 8) {
  const out: number[] = [];
  for (let i = 0; i < CANDLES.length; i++) {
    let g = 0, l = 0;
    for (let k = Math.max(1, i - period + 1); k <= i; k++) {
      const d = CANDLES[k].c - CANDLES[k - 1].c;
      if (d > 0) g += d; else l -= d;
    }
    out.push(i === 0 ? 50 : 100 - 100 / (1 + g / (l || 0.0001)));
  }
  return out;
}

export function DemoPlayer() {
  const [scn, setScn] = useState(0);
  const [run, setRun] = useState(0);
  const [playing, setPlaying] = useState(true);
  const [ended, setEnded] = useState(false);
  const [stage, setStage] = useState(-1);

  const [lines, setLines] = useState<Line[]>([]);
  const [prompt, setPrompt] = useState("");

  // the apps
  const [exp, setExp] = useState({ vendor: "", amount: "", date: "", focus: "", sent: false, reading: false });
  const [chart, setChart] = useState({ dialog: false, query: "", rsi: false });
  const [x, setX] = useState({ text: "", focus: false, picker: false, media: false, posted: false });

  // the HUD
  const [reticle, setReticle] = useState({ x: 0, y: 0, show: false });
  const [lock, setLock] = useState<(Rect & { label: string; right: boolean }) | null>(null);
  const [ripple, setRipple] = useState({ k: 0, x: 0, y: 0 });
  const [scan, setScan] = useState(0);
  const [person, setPerson] = useState({ x: 0, y: 0, show: false, k: 0 });
  const [hi, setHi] = useState<(Rect & { caption: string }) | null>(null);
  const [stamp, setStamp] = useState<{ x: number; y: number; k: number } | null>(null);
  const [acting, setActing] = useState(false);

  const screen = useRef<HTMLDivElement>(null);
  const targets = useRef<Record<string, HTMLElement | null>>({});
  const paused = useRef(false);
  const visible = useRef(false);
  const root = useRef<HTMLDivElement>(null);

  const ref = (id: string) => (el: HTMLElement | null) => {
    targets.current[id] = el;
  };

  useEffect(() => {
    paused.current = !playing;
  }, [playing]);

  // Play only while on screen.
  useEffect(() => {
    const io = new IntersectionObserver(([e]) => (visible.current = e.isIntersecting), { threshold: 0.25 });
    io.observe(root.current!);
    return () => io.disconnect();
  }, []);

  const reset = useCallback(() => {
    setLines([]);
    setPrompt("");
    setStage(-1);
    setEnded(false);
    setExp({ vendor: "", amount: "", date: "", focus: "", sent: false, reading: false });
    setChart({ dialog: false, query: "", rsi: false });
    setX({ text: "", focus: false, picker: false, media: false, posted: false });
    setReticle((r) => ({ ...r, show: false }));
    setLock(null);
    setHi(null);
    setStamp(null);
    setPerson((p) => ({ ...p, show: false }));
    setActing(false);
  }, []);

  useEffect(() => {
    let alive = true;
    reset();

    const wait = (ms: number) =>
      new Promise<void>((resolve, reject) => {
        let left = ms, last = performance.now();
        const step = () => {
          if (!alive) return reject(new Error("stop"));
          const now = performance.now();
          if (!paused.current && visible.current && !stopped()) left -= now - last;
          last = now;
          if (left <= 0) resolve();
          else setTimeout(step, Math.min(left, 50));
        };
        step();
      });

    const rectOf = (id: string): Rect => {
      const el = targets.current[id], box = screen.current?.getBoundingClientRect();
      if (!el || !box) return { x: 0, y: 0, w: 0, h: 0 };
      const r = el.getBoundingClientRect();
      return { x: r.left - box.left, y: r.top - box.top, w: r.width, h: r.height };
    };
    const toScreen = (r: Rect) => {
      const box = screen.current?.getBoundingClientRect();
      const sx = box ? ((r.x + r.w / 2) / box.width) * 1920 : 0;
      const sy = box ? ((r.y + r.h / 2) / box.height) * 1080 : 0;
      return [Math.round(sx), Math.round(sy)] as const;
    };
    let seed = 5;
    const push = (l: Line) => setLines((ls) => [...ls, l].slice(-11));
    const tool = (name: string, args = "") => {
      seed = (seed * 37 + 11) % 101;
      const base = name.endsWith("screenshot") ? 210 : name.endsWith("type_text") ? 160 : 60;
      push({ kind: "tool", text: `${name}(${args})`, ms: base + seed * 2 });
    };

    const ask = async (text: string) => {
      for (let i = 1; i <= text.length; i++) {
        setPrompt(text.slice(0, i));
        await wait(24);
      }
      await wait(300);
      setPrompt("");
      push({ kind: "user", text });
      await wait(450);
    };
    const look = async () => {
      tool("openhud.screenshot");
      setScan((k) => k + 1);
      await wait(950);
    };
    const aim = async (id: string, label: string) => {
      const r = rectOf(id);
      setLock(null);
      setReticle({ x: r.x + r.w / 2, y: r.y + r.h / 2, show: true });
      await wait(720);
      const [sx, sy] = toScreen(r);
      const wide = screen.current?.getBoundingClientRect().width ?? 0;
      setLock({ ...r, label: `${label}  X ${pad4(sx)} Y ${pad4(sy)}`, right: r.x + r.w / 2 > wide * 0.55 });
      await wait(380);
      return `${sx}, ${sy}`;
    };
    const click = async (id: string, label: string) => {
      const at = await aim(id, label);
      const r = rectOf(id);
      setRipple((p) => ({ k: p.k + 1, x: r.x + r.w / 2, y: r.y + r.h / 2 }));
      tool("openhud.click", at);
      await wait(260);
    };
    const typeText = async (value: string, set: (v: string) => void, speed = 30) => {
      tool("openhud.type_text", `"${value.length > 28 ? value.slice(0, 26) + "…" : value}"`);
      for (let i = 1; i <= value.length; i++) {
        set(value.slice(0, i));
        await wait(speed);
      }
      await wait(200);
    };
    const check = async (id: string, text: string) => {
      await look();
      const r = rectOf(id);
      setStamp({ x: r.x + r.w, y: r.y, k: Date.now() });
      push({ kind: "result", text });
      await wait(700);
    };
    const highlight = async (id: string, caption: string) => {
      const r = rectOf(id);
      tool("openhud.highlight", `"${caption}"`);
      setHi({ ...r, caption });
      await wait(900);
    };
    const personClick = async (id: string) => {
      const r = rectOf(id);
      setPerson((p) => ({ ...p, x: r.x + Math.min(r.w / 2, 40), y: r.y + r.h / 2, show: true }));
      await wait(900);
      setPerson((p) => ({ ...p, k: p.k + 1 }));
      await wait(260);
    };
    const hudOff = () => {
      setActing(false);
      setLock(null);
      setReticle((r) => ({ ...r, show: false }));
    };

    const scripts: Record<Id, () => Promise<void>> = {
      do: async () => {
        await ask(SCENARIOS[0].prompt);
        setStage(0);
        tool("read_file", "invoice-nov.pdf");
        setExp((e) => ({ ...e, reading: true }));
        await wait(1100);
        setExp((e) => ({ ...e, reading: false }));
        push({ kind: "result", text: "Atlas Print Co., €412.80, 3 Nov 2026" });
        await wait(400);
        setStage(1);
        setActing(true);
        await look();
        setStage(2);
        await click("vendor", "Vendor");
        setExp((e) => ({ ...e, focus: "vendor" }));
        await typeText("Atlas Print Co.", (v) => setExp((e) => ({ ...e, vendor: v })));
        setStage(3);
        await click("amount", "Amount");
        setExp((e) => ({ ...e, focus: "amount" }));
        await typeText("412.80", (v) => setExp((e) => ({ ...e, amount: v })), 45);
        setStage(4);
        await click("date", "Date");
        setExp((e) => ({ ...e, focus: "date" }));
        await typeText("2026-11-03", (v) => setExp((e) => ({ ...e, date: v })), 40);
        setStage(5);
        await click("submit", "Submit expense");
        setExp((e) => ({ ...e, focus: "", sent: true }));
        await wait(300);
        setStage(6);
        await check("banner", "“Expense submitted” is on screen");
        hudOff();
        push({ kind: "say", text: "Done. Submitted €412.80 to Atlas Print Co." });
      },
      teach: async () => {
        await ask(SCENARIOS[1].prompt);
        setStage(0);
        await look();
        push({ kind: "say", text: "I'll point, you click. Nothing is clicked for you." });
        await wait(500);
        setStage(1);
        await highlight("indicators", "Click Indicators");
        await personClick("indicators");
        setHi(null);
        setChart((c) => ({ ...c, dialog: true }));
        await wait(500);
        await look();
        setStage(2);
        await highlight("search", "Type RSI here");
        await personClick("search");
        for (const v of ["R", "RS", "RSI"]) {
          setChart((c) => ({ ...c, query: v }));
          await wait(180);
        }
        setHi(null);
        await wait(400);
        await look();
        setStage(3);
        await highlight("rsi-row", "Pick Relative Strength Index");
        await personClick("rsi-row");
        setHi(null);
        setChart((c) => ({ ...c, dialog: false, rsi: true }));
        await wait(600);
        setStage(4);
        await check("rsi-pane", "The RSI pane is under the chart");
        push({ kind: "say", text: "That's RSI. Above 70 is overbought, below 30 oversold." });
        setPerson((p) => ({ ...p, show: false }));
      },
      post: async () => {
        await ask(SCENARIOS[2].prompt);
        setStage(0);
        setActing(true);
        tool("openhud.open_url", "x.com/compose/post");
        await wait(700);
        await look();
        setStage(1);
        await click("compose", "What's happening?");
        setX((v) => ({ ...v, focus: true }));
        await typeText(POST, (t) => setX((v) => ({ ...v, text: t })), 14);
        setStage(2);
        await click("media", "Add media");
        setX((v) => ({ ...v, picker: true }));
        await wait(450);
        await look();
        await click("file", "demo.mp4");
        setX((v) => ({ ...v, picker: false, media: true }));
        await wait(500);
        setStage(3);
        await click("post", "Post");
        setX((v) => ({ ...v, posted: true }));
        await wait(400);
        setStage(4);
        await check("toast", "“Your post was sent”");
        hudOff();
        push({ kind: "say", text: "Posted, with the video: x.com/you/status/1864…" });
      },
    };

    (async () => {
      try {
        await wait(500);
        await scripts[SCENARIOS[scn].id]();
        setEnded(true);
        await wait(3400);
        setScn((v) => (v + 1) % SCENARIOS.length);
      } catch {
        /* switched or unmounted */
      }
    })();
    return () => {
      alive = false;
    };
  }, [scn, run, reset]);

  const pick = (i: number) => {
    setPlaying(true);
    if (i === scn) setRun((r) => r + 1);
    else setScn(i);
  };

  const s = SCENARIOS[scn];
  const rsiLine = useMemo(() => rsi(), []);
  const lo = Math.min(...CANDLES.map((c) => c.lo)), hiP = Math.max(...CANDLES.map((c) => c.hi));
  const cy = (v: number) => 4 + ((hiP - v) / (hiP - lo)) * 92;
  const progress = ended ? 1 : Math.max(0, stage + 1) / s.marks.length;

  return (
    <div className="dp" ref={root}>
      <div className="dp-tabs" role="tablist" aria-label="Demo scenarios">
        {SCENARIOS.map((v, i) => (
          <button key={v.id} type="button" role="tab" aria-selected={i === scn} className="dp-tab" onClick={() => pick(i)}>
            <code>{v.skill}</code>
            <span>{v.label}</span>
            <i className="dp-tab-bar" style={{ transform: `scaleX(${i === scn ? progress : 0})` }} />
          </button>
        ))}
      </div>

      <div className={`dp-screen${acting ? " dp-acting" : ""}`} ref={screen}>
        <div className="dp-topbar">
          <span className="dp-island"><i />{ended ? "Done" : acting ? "OpenHUD is using your mouse" : s.id === "teach" ? "OpenHUD is pointing, you click" : "Ready"}</span>
        </div>

        {/* the agent */}
        <section className="dp-console" aria-label="The agent's session">
          <header><span className="dp-dots"><i /><i /><i /></span>claude<em>openhud connected</em></header>
          <div className="dp-lines" aria-live="polite">
            {lines.map((l, i) => (
              <p key={i} className={`dp-${l.kind}`}>
                {l.kind === "user" && <><b className="dp-caret">&gt;</b><b className="dp-skill">{s.skill}</b> {l.text}</>}
                {l.kind === "tool" && <><i className="dp-dot" /><span>{l.text}</span><em>{l.ms}ms</em></>}
                {l.kind === "result" && <><span className="dp-elbow">⎿</span>{l.text}</>}
                {l.kind === "say" && l.text}
              </p>
            ))}
            <p className="dp-prompt"><b className="dp-caret">&gt;</b>{prompt}<span className="dp-cursor" /></p>
          </div>
        </section>

        {/* the app being used */}
        <section className="dp-app" aria-label="The app on screen">
          {s.id === "do" && (
            <div className="dp-exp">
              <div className={`dp-pdf${exp.reading ? " dp-reading" : ""}`}>
                <b>INVOICE</b><span>Atlas Print Co.</span><span>3 Nov 2026</span><span className="dp-pdf-total">€412.80</span>
                <em>invoice-nov.pdf</em>
              </div>
              <div className="dp-form">
                <h4>New expense</h4>
                <label>Vendor<span ref={ref("vendor")} className={exp.focus === "vendor" ? "dp-f dp-focus" : "dp-f"}>{exp.vendor}</span></label>
                <div className="dp-row">
                  <label>Amount, EUR<span ref={ref("amount")} className={exp.focus === "amount" ? "dp-f dp-focus" : "dp-f"}>{exp.amount}</span></label>
                  <label>Date<span ref={ref("date")} className={exp.focus === "date" ? "dp-f dp-focus" : "dp-f"}>{exp.date}</span></label>
                </div>
                <span ref={ref("submit")} className="dp-btn">Submit expense</span>
                <p ref={ref("banner")} className={`dp-banner${exp.sent ? " dp-on" : ""}`}>✓ Expense submitted</p>
              </div>
            </div>
          )}

          {s.id === "teach" && (
            <div className="dp-chart">
              <div className="dp-chart-bar">
                <b>AAPL</b><span>1D</span>
                <span ref={ref("indicators")} className="dp-chip">ƒx Indicators</span>
                <span className="dp-chip dp-mute">Alerts</span>
              </div>
              <svg className="dp-candles" viewBox="0 0 230 100" preserveAspectRatio="none" aria-hidden="true">
                {CANDLES.map((c, i) => {
                  const up = c.c >= c.o, xx = 3 + i * 4.9;
                  return (
                    <g key={i} className={up ? "up" : "down"}>
                      <line x1={xx} x2={xx} y1={cy(c.hi)} y2={cy(c.lo)} />
                      <rect x={xx - 1.4} width="2.8" y={cy(Math.max(c.o, c.c))} height={Math.max(0.8, Math.abs(cy(c.o) - cy(c.c)))} />
                    </g>
                  );
                })}
              </svg>
              <div ref={ref("rsi-pane")} className={`dp-rsi${chart.rsi ? " dp-on" : ""}`}>
                <span>RSI 8</span>
                <svg viewBox="0 0 230 40" preserveAspectRatio="none" aria-hidden="true">
                  <line x1="0" x2="230" y1="12" y2="12" className="lvl" />
                  <line x1="0" x2="230" y1="28" y2="28" className="lvl" />
                  <polyline points={rsiLine.map((v, i) => `${3 + i * 4.9},${40 - (v / 100) * 40}`).join(" ")} />
                </svg>
              </div>
              {chart.dialog && (
                <div className="dp-dialog">
                  <h4>Indicators</h4>
                  <span ref={ref("search")} className="dp-f dp-search">{chart.query || <em>Search</em>}</span>
                  <ul>
                    {["Relative Strength Index", "MACD", "Bollinger Bands", "Moving Average"]
                      .filter((n) => !chart.query || n.startsWith("Relative"))
                      .map((n) => <li key={n} ref={n.startsWith("Relative") ? ref("rsi-row") : undefined}>{n}</li>)}
                  </ul>
                </div>
              )}
            </div>
          )}

          {s.id === "post" && (
            <div className="dp-x">
              <div className="dp-x-compose">
                <span className="dp-avatar" />
                <div>
                  <p ref={ref("compose")} className={`dp-x-text${x.focus ? " dp-focus" : ""}`}>{x.text || <em>What&apos;s happening?</em>}</p>
                  {x.media && <div className="dp-x-media"><span>▶</span>demo.mp4 · 0:24</div>}
                  <div className="dp-x-bar">
                    <span ref={ref("media")} className="dp-x-icon">▣</span>
                    <span className="dp-x-icon">GIF</span>
                    <span ref={ref("post")} className="dp-x-post">Post</span>
                  </div>
                </div>
              </div>
              {x.picker && (
                <div className="dp-picker">
                  <h4>~/Videos</h4>
                  <p>intro.mov</p>
                  <p ref={ref("file")}>demo.mp4</p>
                  <p>screen-rec-2.mkv</p>
                </div>
              )}
              <p ref={ref("toast")} className={`dp-toast${x.posted ? " dp-on" : ""}`}>Your post was sent · View</p>
            </div>
          )}
        </section>

        {/* the HUD over everything */}
        <div className="dp-hud" aria-hidden="true">
          {scan > 0 && <div key={`scan${scan}`} className="dp-scan"><span>LOOK · 1280×720</span></div>}
          {lock && (
            <div className={`dp-lock${lock.right ? " dp-lock-r" : ""}`} style={{ left: lock.x - 6, top: lock.y - 6, width: lock.w + 12, height: lock.h + 12 }}>
              <i /><i /><i /><i /><span>{lock.label}</span>
            </div>
          )}
          <div className={`dp-reticle${reticle.show ? " dp-vis" : ""}`} style={{ transform: `translate(${reticle.x}px, ${reticle.y}px)` }}>
            <svg viewBox="-20 -20 40 40"><circle r="13" /><circle r="3" className="core" /><path d="M0 -19v6M0 13v6M-19 0h6M13 0h6" /></svg>
          </div>
          {ripple.k > 0 && <span key={`rp${ripple.k}`} className="dp-ripple" style={{ left: ripple.x, top: ripple.y }} />}
          {hi && (
            <div className="dp-hi" style={{ left: hi.x - 8, top: hi.y - 8, width: hi.w + 16, height: hi.h + 16 }}>
              <span>{hi.caption}</span>
            </div>
          )}
          <div className={`dp-person${person.show ? " dp-vis" : ""}`} style={{ transform: `translate(${person.x}px, ${person.y}px)` }}>
            <svg viewBox="0 0 24 24"><path d="M3 2l7 19 2.6-7.4L20 11z" /></svg>
            <em>you</em>
            {person.k > 0 && <span key={`pk${person.k}`} className="dp-ripple dp-ripple-person" />}
          </div>
          {stamp && <span key={stamp.k} className="dp-stamp" style={{ left: stamp.x, top: stamp.y }}>✓ verified</span>}
        </div>
      </div>

      <div className="dp-controls">
        <button type="button" className="dp-play" onClick={() => setPlaying((p) => !p)} aria-label={playing ? "Pause the demo" : "Play the demo"}>
          {playing ? <Pause size={15} strokeWidth={1.8} /> : <Play size={15} strokeWidth={1.8} />}
        </button>
        <ol className="dp-marks">
          {s.marks.map((m, i) => (
            <li key={m} className={ended || i < stage ? "done" : i === stage ? "now" : ""}>{m}</li>
          ))}
        </ol>
        <button type="button" className="dp-play" onClick={() => pick(scn)} aria-label="Replay this scenario">
          <RotateCcw size={15} strokeWidth={1.8} />
        </button>
      </div>
    </div>
  );
}
