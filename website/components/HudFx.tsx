"use client";

import { useEffect, useRef, useState } from "react";
import { stopped } from "./stop";

/**
 * The HUD layer for every page (mounted once in app/layout.tsx):
 *  - each section's heading gets a tag ("02 // SKILLS") and decodes from noise the first
 *    time it scrolls into view, while a scan line sweeps the section;
 *  - cards, videos, code and tables get corner brackets that snap in on view and lock
 *    tighter on hover;
 *  - a tracker in the corner: a ring that fills as you scroll, the section you're in and
 *    a Y readout. Click it to go back to the top.
 * Reduced motion: tags and brackets show, nothing decodes or sweeps.
 */

const FRAMES = [
  ".skill", ".provider", ".app-visual", ".hero-clip",
  "main pre:not(.install pre)", ".docs-cards > *", ".docs-table", ".estop-plinth",
].join(",");

const GLYPHS = "ABCDEFGHJKLMNPQRSTUVWXYZ0123456789#%/<>";
const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;

function decode(el: HTMLElement, final: string, ms: number) {
  if (reduced() || stopped()) {
    el.textContent = final;
    return;
  }
  const start = performance.now();
  const tick = (now: number) => {
    const k = Math.min(1, (now - start) / ms);
    const fixed = Math.floor(k * final.length);
    el.textContent = final
      .split("")
      .map((c, i) => (i < fixed || c === " " || c === "," ? c : GLYPHS[(Math.random() * GLYPHS.length) | 0]))
      .join("");
    if (k < 1) requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
}

type Sec = { el: HTMLElement; tag: string; name: string };

export function HudFx() {
  const [label, setLabel] = useState("00 // OPENHUD");
  const [shown, setShown] = useState(label);
  const ring = useRef<SVGCircleElement>(null);
  const yRead = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    // Sections: the hero, then every section with a heading.
    const secs: Sec[] = [];
    const all = Array.from(document.querySelectorAll<HTMLElement>("main section"));
    let n = 0;
    for (const el of all) {
      const h2 = el.querySelector<HTMLElement>(":scope h2, :scope > div h2");
      const isHero = el.classList.contains("hero");
      if (!h2 && !isHero) continue;
      if (h2 && h2.closest("section") !== el) continue;
      const tocName = el.id && document.querySelector(`.docs-toc a[href="#${el.id}"]`)?.textContent;
      const name = (isHero ? "OpenHUD" : tocName || el.id || el.className.split(" ")[0] || "section")
        .replace(/-/g, " ")
        .toUpperCase();
      const tag = `${String(n).padStart(2, "0")} // ${name}`;
      n++;
      secs.push({ el, tag, name });
      if (h2) {
        h2.dataset.hudTag = tag;
        h2.dataset.hudText = h2.textContent || "";
        h2.classList.add("hud-h");
      }
    }

    // Brackets on cards, videos, code and tables.
    const frames = Array.from(document.querySelectorAll<HTMLElement>(FRAMES));
    for (const f of frames) {
      if (getComputedStyle(f).position === "static") f.style.position = "relative";
      f.classList.add("hud-frame");
    }

    const seen = new WeakSet<Element>();
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          if (!e.isIntersecting || seen.has(e.target)) continue;
          seen.add(e.target);
          const el = e.target as HTMLElement;
          el.classList.add("hud-in");
          if (el.classList.contains("hud-h") && el.dataset.hudText) decode(el, el.dataset.hudText, 700);
          if (el.tagName === "SECTION" && !reduced()) {
            const scan = document.createElement("div");
            scan.className = "hud-scan";
            scan.style.setProperty("--h", `${el.offsetHeight}px`);
            scan.setAttribute("aria-hidden", "true");
            scan.addEventListener("animationend", () => scan.remove());
            el.appendChild(scan);
          }
        }
      },
      { threshold: 0.18 },
    );
    for (const s of secs) {
      io.observe(s.el);
      const h2 = s.el.querySelector(".hud-h");
      if (h2) io.observe(h2);
    }
    frames.forEach((f) => io.observe(f));

    // The tracker: progress ring, current section, Y readout; and the docs' active link.
    const C = 2 * Math.PI * 22;
    let raf = 0, last = -1, current = "";
    const update = () => {
      raf = 0;
      const max = document.documentElement.scrollHeight - innerHeight;
      const p = max > 0 ? Math.min(1, scrollY / max) : 0;
      if (ring.current) ring.current.style.strokeDashoffset = String(C * (1 - p));
      if (yRead.current) yRead.current.textContent = String(Math.round(scrollY)).padStart(5, "0");
      // the hero has its own HUD; the tracker comes in once you've scrolled past it
      document.documentElement.classList.toggle("hud-past-hero", scrollY > innerHeight * 0.6);
      let active = secs[0];
      for (const s of secs) if (s.el.getBoundingClientRect().top < innerHeight * 0.4) active = s;
      if (active && active.tag !== current) {
        current = active.tag;
        setLabel(active.tag);
        document.querySelectorAll(".docs-toc a").forEach((a) => {
          if (a.getAttribute("href") === `#${active.el.id}`) a.setAttribute("data-active", "");
          else a.removeAttribute("data-active");
        });
      }
      last = p;
    };
    const onScroll = () => {
      if (!raf) raf = requestAnimationFrame(update);
    };
    update();
    addEventListener("scroll", onScroll, { passive: true });
    addEventListener("resize", onScroll);
    void last;
    return () => {
      io.disconnect();
      removeEventListener("scroll", onScroll);
      removeEventListener("resize", onScroll);
      cancelAnimationFrame(raf);
    };
  }, []);

  // The tracker's label decodes when the section changes.
  useEffect(() => {
    if (reduced()) {
      setShown(label);
      return;
    }
    const start = performance.now();
    let raf = 0;
    const tick = (now: number) => {
      const k = Math.min(1, (now - start) / 380);
      const fixed = Math.floor(k * label.length);
      setShown(label.split("").map((c, i) => (i < fixed || c === " " || c === "/" ? c : GLYPHS[(Math.random() * GLYPHS.length) | 0])).join(""));
      if (k < 1) raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [label]);

  return (
    <button type="button" className="hud-tracker" onClick={() => scrollTo({ top: 0, behavior: reduced() ? "auto" : "smooth" })} aria-label="Back to the top">
      <span className="hud-tracker-text" aria-hidden="true">
        <b>{shown}</b>
        <span>Y <span ref={yRead}>00000</span></span>
      </span>
      <svg viewBox="-30 -30 60 60" width="52" height="52" aria-hidden="true">
        <circle r="27" className="ht-scale" />
        <circle r="22" className="ht-track" />
        <circle ref={ring} r="22" className="ht-fill" strokeDasharray={2 * Math.PI * 22} strokeDashoffset={2 * Math.PI * 22} transform="rotate(-90)" />
        <circle r="3" className="ht-core" />
      </svg>
    </button>
  );
}
