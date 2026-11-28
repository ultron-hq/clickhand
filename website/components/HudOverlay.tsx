"use client";

import { useEffect, useRef, useState } from "react";
import { pausableWait, stopped } from "./stop";

/**
 * The HUD around the hero's reticle: rings that draw themselves in and counter-rotate,
 * a radar sweep, and an agent's loop played out on it. Each round it looks, a target
 * appears, the brackets lock onto it, it clicks (the target presses and a ripple
 * goes out), then it checks. The readouts decode like a heads-up display.
 *
 * It sits exactly on the reticle MaskField draws (MaskField publishes the position as
 * --hud-cx / --hud-cy / --hud-size on the hero). Pauses with the page's kill switch;
 * reduced motion shows one still, locked frame.
 */

type Target = { label: string; x: number; y: number; px: number; py: number; typed?: string };

// Targets around the reticle, in units of the reticle's size from its centre.
const TARGETS: Target[] = [
  { label: "Export PNG", x: 0.46, y: -0.2, px: 1184, py: 92 },
  { label: "Submit expense", x: 0.16, y: 0.53, px: 806, py: 655, typed: "Atlas Print Co." },
  { label: "Add indicator", x: -0.45, y: 0.2, px: 214, py: 61 },
  { label: "Post", x: -0.2, y: -0.53, px: 1031, py: 588 },
];

type Phase = "boot" | "look" | "lock" | "act" | "check";

const GLYPHS = "ABCDEFGHJKLMNPQRSTUVWXYZ0123456789#%/<>";

/** Text that resolves from noise, left to right. */
function Decode({ text, ms = 420 }: { text: string; ms?: number }) {
  const [shown, setShown] = useState(text);
  useEffect(() => {
    if (matchMedia("(prefers-reduced-motion: reduce)").matches) {
      setShown(text);
      return;
    }
    const start = performance.now();
    let raf = 0;
    const tick = (now: number) => {
      const k = Math.min(1, (now - start) / ms);
      const fixed = Math.floor(k * text.length);
      setShown(
        text
          .split("")
          .map((c, i) => (i < fixed || c === " " ? c : GLYPHS[(Math.random() * GLYPHS.length) | 0]))
          .join(""),
      );
      if (k < 1) raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [text, ms]);
  return <>{shown}</>;
}

const pad = (n: number) => String(Math.round(n)).padStart(4, "0");

export function HudOverlay() {
  const [phase, setPhase] = useState<Phase>("boot");
  const [i, setI] = useState(0);
  const [clicks, setClicks] = useState(0);
  const root = useRef<HTMLDivElement>(null);

  // The loop: look → lock → act → check, one target per round.
  useEffect(() => {
    if (matchMedia("(prefers-reduced-motion: reduce)").matches) {
      setPhase("lock");
      return;
    }
    let alive = true;
    const wait = (ms: number) => pausableWait(ms, () => alive);
    (async () => {
      try {
        await wait(2600); // after the dots have flown in and the rings have drawn
        for (let n = 0; alive; n++) {
          setI(n % TARGETS.length);
          setPhase("look");
          await wait(1100);
          setPhase("lock");
          await wait(1200);
          setPhase("act");
          setClicks((c) => c + 1);
          await wait(TARGETS[n % TARGETS.length].typed ? 1500 : 900);
          setPhase("check");
          await wait(1300);
        }
      } catch {
        /* unmounted */
      }
    })();
    return () => {
      alive = false;
    };
  }, []);

  // Nothing moves while the hero is off screen.
  useEffect(() => {
    const el = root.current!;
    const io = new IntersectionObserver(([e]) => el.classList.toggle("hud-off", !e.isIntersecting));
    io.observe(el);
    return () => io.disconnect();
  }, []);

  // A little parallax with the pointer.
  useEffect(() => {
    const el = root.current!;
    if (matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    let raf = 0;
    const cur = { x: 0, y: 0 }, goal = { x: 0, y: 0 };
    const move = (e: PointerEvent) => {
      goal.x = (e.clientX / innerWidth - 0.5) * 18;
      goal.y = (e.clientY / innerHeight - 0.5) * 14;
    };
    const loop = () => {
      if (!stopped() && !el.classList.contains("hud-off")) {
        cur.x += (goal.x - cur.x) * 0.06;
        cur.y += (goal.y - cur.y) * 0.06;
        el.style.setProperty("--px", `${cur.x.toFixed(2)}px`);
        el.style.setProperty("--py", `${cur.y.toFixed(2)}px`);
      }
      raf = requestAnimationFrame(loop);
    };
    addEventListener("pointermove", move);
    raf = requestAnimationFrame(loop);
    return () => {
      removeEventListener("pointermove", move);
      cancelAnimationFrame(raf);
    };
  }, []);

  const t = TARGETS[i];
  const shown = phase !== "boot" && phase !== "look";
  const step =
    phase === "boot" ? ["BOOT", "hands online"]
    : phase === "look" ? ["LOOK", "screenshot 1280×720"]
    : phase === "lock" ? ["LOCK", t.label]
    : phase === "act" ? (t.typed ? ["TYPE", `${t.typed.length} chars`] : ["ACT", `click (${t.px}, ${t.py})`])
    : ["CHECK", "verified"];

  return (
    <div ref={root} className={`hud hud-${phase}`} aria-hidden="true">
      <div className="hud-sweep" />
      {/* each spinning ring is its own layer, rotated on the GPU */}
      <svg className="hud-rings" viewBox="-300 -300 600 600">
        {/* outer scale: 120 ticks, every tenth long */}
        <circle className="hud-draw" r="286" pathLength={1} />
        {Array.from({ length: 120 }, (_, k) => (
          <line key={k} x1="0" y1={-286} x2="0" y2={k % 10 === 0 ? -272 : -280} transform={`rotate(${k * 3})`} className={k % 10 === 0 ? "tick tick-long" : "tick"} />
        ))}
      </svg>
      <svg className="hud-rings hud-spin hud-rev" viewBox="-300 -300 600 600">
        {/* three heavy arcs, counter-rotating; the glow is a wider faint stroke, not a filter */}
        {[0, 120, 240].map((a) => (
          <g key={a} transform={`rotate(${a})`}>
            <path className="hud-arc-glow hud-draw" pathLength={1} d="M 0 -258 A 258 258 0 0 1 199.6 -163.4" />
            <path className="hud-arc hud-draw" pathLength={1} d="M 0 -258 A 258 258 0 0 1 199.6 -163.4" />
          </g>
        ))}
      </svg>
      <svg className="hud-rings hud-spin hud-mid" viewBox="-300 -300 600 600">
        <circle className="hud-dots" r="236" />
      </svg>
      <svg className="hud-rings" viewBox="-300 -300 600 600">
        {/* a line from the core to what it's aiming at */}
        <line key={`l${i}`} className="hud-link" x1="0" y1="0" x2={t.x * (600 / 1.12)} y2={t.y * (600 / 1.12)} data-on={shown || undefined} />
        {/* the click: a ripple from the core */}
        <circle key={clicks} className="hud-ripple" r="40" />
      </svg>

      {/* the target and the brackets that lock onto it */}
      <div className="hud-target" style={{ ["--tx" as string]: t.x, ["--ty" as string]: t.y }} data-on={shown || undefined}>
        <span className="hud-chip" data-press={phase === "act" || undefined}>
          {phase === "act" && t.typed ? <Decode text={t.typed} ms={900} /> : t.label}
        </span>
        <span className="hud-brackets" data-lock={shown || undefined}>
          <i /><i /><i /><i />
        </span>
        {phase === "act" && <span key={`r${clicks}`} className="hud-tap" />}
      </div>

      {/* readouts */}
      <div className="hud-read hud-tl">
        <b>OPENHUD</b>
        <span><Decode text={phase === "boot" ? "LINKING HANDS…" : "HANDS ONLINE"} /></span>
      </div>
      <div className="hud-read hud-tr">
        <b className="hud-step"><Decode key={step[0]} text={step[0]} ms={260} /></b>
        <span><Decode key={step[1]} text={step[1]} /></span>
      </div>
      <div className="hud-read hud-bl">
        <span>X <Decode key={`x${i}${shown}`} text={shown ? pad(t.px) : "----"} /></span>
        <span>Y <Decode key={`y${i}${shown}`} text={shown ? pad(t.py) : "----"} /></span>
      </div>
      <div className="hud-read hud-br">
        <span>KILL SWITCH</span>
        <b>ARMED</b>
      </div>
    </div>
  );
}
