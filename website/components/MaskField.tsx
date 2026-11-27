"use client";

import { useEffect, useRef } from "react";
import { stopped } from "./stop";

type Dot = {
  tx: number; ty: number;   // home position in the mask (0..1)
  x: number; y: number;     // current position (px)
  vx: number; vy: number;
  light: number;            // 0..1 brightness from the logo
  eye: boolean;
  delay: number;            // when it starts flying home (0..1 of the intro)
  seed: number;
};

type Beam = { sx: number; sy: number; c1: number; c2: number; eye: 0 | 1; speed: number; phase: number; width: number };

const GRID = 96;
const INTRO_MS = 2200;
const SAMPLES = 28;
const ease = (t: number) => 1 - Math.pow(1 - t, 4);

// The brand gradient, from the mask's eye light: ink, crimson, ember, amber.
const STOPS: [number, [number, number, number]][] = [
  [0, [20, 23, 34]],
  [0.32, [74, 18, 24]],
  [0.58, [196, 48, 42]],
  [0.8, [255, 122, 47]],
  [1, [255, 205, 140]],
];
function heat(t: number): string {
  t = Math.max(0, Math.min(1, t));
  for (let i = 1; i < STOPS.length; i++) {
    const [p1, c1] = STOPS[i];
    const [p0, c0] = STOPS[i - 1];
    if (t <= p1) {
      const k = (t - p0) / (p1 - p0);
      return `rgb(${Math.round(c0[0] + (c1[0] - c0[0]) * k)}, ${Math.round(c0[1] + (c1[1] - c0[1]) * k)}, ${Math.round(c0[2] + (c1[2] - c0[2]) * k)})`;
    }
  }
  return "rgb(255, 205, 140)";
}

// Eye centres in the 64×64 logo.
// The two click ripples above the fingertip: they light up last and cast the beams.
const EYES = [{ x: 24 / 64, y: 9.5 / 64 }, { x: 21 / 64, y: 5.5 / 64 }];
// The mask's outline in the 64×64 logo: beams stop at the edge of the face.
const OUTLINE = "M27 11 36 11 36 26 55 30 55 44 45 57 32 57 12 40 14 33 27 41Z";

/**
 * The hero: the Clickhand mask in points, lit by its own red light, with lines of light
 * sweeping in from the edge of the page and converging on its eyes. The dots fly in
 * and lock into the face on load; the eyes then follow the pointer and the dots part
 * around it. Still when reduced motion is on.
 */
export function MaskField() {
  const canvas = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const cv = canvas.current!;
    const ctx = cv.getContext("2d")!;
    const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
    let dots: Dot[] = [];
    let beams: Beam[] = [];
    let w = 0, h = 0, dpr = 1, size = 0, ox = 0, oy = 0, wide = true;
    let start = 0, raf = 0, visible = true;
    const pointer = { x: -9999, y: -9999, active: false };
    const gaze = { x: 0, y: 0 };

    const layout = () => {
      dpr = Math.min(devicePixelRatio || 1, 2);
      const r = cv.getBoundingClientRect();
      w = r.width; h = r.height;
      cv.width = Math.round(w * dpr); cv.height = Math.round(h * dpr);
      wide = w >= 960;
      if (wide) {
        size = Math.min(h * 0.8, w * 0.4, 640);
        ox = Math.min(w * 0.74, w / 2 + 600 - size * 0.55) - size / 2;
        oy = (h - size) / 2;
      } else {
        size = Math.min(w * 0.72, 340);
        ox = (w - size) / 2;
        oy = 24;
      }
      beams = Array.from({ length: wide ? 34 : 18 }, (_, i) => {
        // on small screens the text sits under the mask, so light only comes from the right, above it
        const fromRight = !wide || i % 3 !== 0;
        return {
          sx: fromRight ? w + 40 : ox + size * (0.2 + Math.random() * 1.4),
          sy: fromRight ? (wide ? h * (-0.1 + Math.random() * 1.2) : (oy + size) * (-0.2 + Math.random() * 1.1)) : h + 40,
          c1: Math.random(), c2: Math.random(),
          eye: (i % 2) as 0 | 1,
          speed: 0.00006 + Math.random() * 0.00008,
          phase: Math.random(),
          width: 0.6 + Math.random() * 0.9,
        };
      });
    };

    const img = new Image();
    img.src = "/hero.svg";
    img.onload = () => {
      const s = document.createElement("canvas");
      s.width = GRID; s.height = GRID;
      const sc = s.getContext("2d", { willReadFrequently: true })!;
      sc.drawImage(img, 0, 0, GRID, GRID);
      const px = sc.getImageData(0, 0, GRID, GRID).data;
      layout();
      const next: Dot[] = [];
      for (let y = 0; y < GRID; y++) {
        for (let x = 0; x < GRID; x++) {
          const i = (y * GRID + x) * 4;
          if (px[i + 3] / 255 < 0.35) continue;
          const [r, g, b] = [px[i], px[i + 1], px[i + 2]];
          const eye = r > 170 && g < 140 && b < 140;
          const light = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
          const angle = Math.random() * Math.PI * 2;
          const dist = (0.5 + Math.random() * 0.8) * Math.max(w, h);
          next.push({
            tx: (x + 0.5) / GRID, ty: (y + 0.5) / GRID,
            x: ox + size / 2 + Math.cos(angle) * dist, y: oy + size / 2 + Math.sin(angle) * dist,
            vx: 0, vy: 0, light, eye,
            delay: eye ? 0.55 + Math.random() * 0.1 : Math.random() * 0.45,
            seed: Math.random(),
          });
        }
      }
      dots = next;
      start = performance.now();
      if (reduced) draw(start + INTRO_MS * 2, true);
      else raf = requestAnimationFrame(loop);
    };

    const eyePoint = (i: 0 | 1) => ({ x: ox + EYES[i].x * size + gaze.x, y: oy + EYES[i].y * size + gaze.y });

    const drawBeams = (now: number, t: number, still: boolean) => {
      const reveal = still ? 1 : Math.min(1, Math.max(0, (t - 0.35) / 0.65));
      if (reveal <= 0) return;
      ctx.save();
      const clip = new Path2D();
      clip.rect(0, 0, w, h);
      clip.addPath(new Path2D(OUTLINE), new DOMMatrix().translate(ox, oy).scale(size / 64));
      ctx.clip(clip, "evenodd");
      ctx.lineCap = "round";
      for (const b of beams) {
        const end = eyePoint(b.eye);
        const wob = still ? 0 : Math.sin(now * 0.0004 + b.phase * 6.28) * 40;
        const c1x = b.sx + (end.x - b.sx) * 0.3 + wob, c1y = b.sy + (end.y - b.sy) * (0.1 + b.c1 * 0.3);
        const c2x = b.sx + (end.x - b.sx) * (0.6 + b.c2 * 0.2), c2y = end.y + (b.sy - end.y) * 0.15 - wob * 0.5;
        // sample the curve
        const pts: [number, number][] = [];
        for (let k = 0; k <= SAMPLES; k++) {
          const u = k / SAMPLES, v = 1 - u;
          pts.push([
            v * v * v * b.sx + 3 * v * v * u * c1x + 3 * v * u * u * c2x + u * u * u * end.x,
            v * v * v * b.sy + 3 * v * v * u * c1y + 3 * v * u * u * c2y + u * u * u * end.y,
          ]);
        }
        // the beam: dark at the page edge, heating up toward the eye, stopping short of it
        const grad = ctx.createLinearGradient(b.sx, b.sy, end.x, end.y);
        grad.addColorStop(0, "rgba(196, 48, 42, 0)");
        grad.addColorStop(0.45, `rgba(196, 48, 42, ${0.28 * reveal})`);
        grad.addColorStop(0.8, `rgba(255, 122, 47, ${0.5 * reveal})`);
        grad.addColorStop(0.93, `rgba(255, 205, 140, ${0.35 * reveal})`);
        grad.addColorStop(1, "rgba(255, 205, 140, 0)");
        ctx.strokeStyle = grad;
        ctx.lineWidth = b.width;
        ctx.beginPath();
        pts.forEach(([x, y], k) => (k ? ctx.lineTo(x, y) : ctx.moveTo(x, y)));
        ctx.stroke();
        if (still) continue;
        // a pulse of light travelling into the eye
        const head = ((now * b.speed + b.phase) % 1) * SAMPLES;
        const from = Math.max(0, Math.floor(head) - 3), to = Math.min(SAMPLES, Math.floor(head));
        if (to - from < 1) continue;
        ctx.strokeStyle = `rgba(255, ${150 + Math.round(head * 2)}, 90, ${0.75 * reveal})`;
        ctx.lineWidth = b.width + 0.8;
        ctx.beginPath();
        for (let k = from; k <= to; k++) (k === from ? ctx.moveTo : ctx.lineTo).call(ctx, pts[k][0], pts[k][1]);
        ctx.stroke();
      }
      ctx.restore();
    };

    const draw = (now: number, still = false) => {
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, w, h);
      const t = Math.min(1, (now - start) / INTRO_MS);
      const cell = size / GRID;

      if (pointer.active) {
        const cx = ox + size / 2, cy = oy + size * 0.42;
        const dx = pointer.x - cx, dy = pointer.y - cy;
        const len = Math.hypot(dx, dy) || 1;
        const reach = Math.min(1, len / 400) * cell * 1.6;
        gaze.x += ((dx / len) * reach - gaze.x) * 0.08;
        gaze.y += ((dy / len) * reach - gaze.y) * 0.08;
      } else {
        gaze.x *= 0.94; gaze.y *= 0.94;
      }

      drawBeams(now, t, still);

      for (const d of dots) {
        let hx = ox + d.tx * size, hy = oy + d.ty * size;
        if (d.eye) { hx += gaze.x; hy += gaze.y; }
        if (still) { d.x = hx; d.y = hy; }
        else {
          const local = Math.max(0, Math.min(1, ((t - d.delay) / (1 - d.delay + 0.0001)) * 1.6));
          if (local < 1) {
            const e = ease(local);
            d.x += (hx - d.x) * (0.02 + e * 0.12);
            d.y += (hy - d.y) * (0.02 + e * 0.12);
          } else {
            let fx = (hx - d.x) * 0.09, fy = (hy - d.y) * 0.09;
            if (pointer.active) {
              const dx = d.x - pointer.x, dy = d.y - pointer.y;
              const dist2 = dx * dx + dy * dy, R = 70;
              if (dist2 < R * R) {
                const dist = Math.sqrt(dist2) || 1, push = (1 - dist / R) * 3.2;
                fx += (dx / dist) * push; fy += (dy / dist) * push;
              }
            }
            d.vx = (d.vx + fx) * 0.78; d.vy = (d.vy + fy) * 0.78;
            d.x += d.vx; d.y += d.vy;
          }
        }
        const shimmer = still ? 0 : Math.sin(now * 0.0012 + d.seed * 40) * 0.05;
        // light falls from the eyes: dots nearer the eye line burn a little hotter
        const near = 1 - Math.min(1, Math.abs(d.ty - 0.44) * 2.2);
        const l = Math.max(0, Math.min(1, d.light * 0.85 + near * 0.18 + shimmer));
        const r = Math.max(0.5, cell * (0.22 + l * 0.28));
        if (d.eye) {
          const on = still ? 1 : Math.min(1, Math.max(0, (t - 0.7) / 0.3));
          ctx.fillStyle = `rgba(255, 244, 226, ${0.3 + on * 0.7})`;
        } else {
          ctx.fillStyle = heat(l);
        }
        ctx.fillRect(d.x - r, d.y - r, r * 2, r * 2);
      }
    };

    const loop = (now: number) => {
      if (visible && !stopped()) draw(now);
      raf = requestAnimationFrame(loop);
    };

    const onMove = (e: PointerEvent) => {
      const r = cv.getBoundingClientRect();
      pointer.x = e.clientX - r.left; pointer.y = e.clientY - r.top;
      pointer.active = true;
    };
    const onLeave = () => { pointer.active = false; pointer.x = pointer.y = -9999; };
    const io = new IntersectionObserver(([e]) => { visible = e.isIntersecting; });
    io.observe(cv);
    let first = true;
    const ro = new ResizeObserver(() => {
      if (first) { first = false; return; }
      layout();
      if (reduced && dots.length) draw(performance.now(), true);
    });
    ro.observe(cv);
    window.addEventListener("pointermove", onMove, { passive: true });
    document.addEventListener("pointerleave", onLeave);

    return () => {
      cancelAnimationFrame(raf);
      io.disconnect(); ro.disconnect();
      window.removeEventListener("pointermove", onMove);
      document.removeEventListener("pointerleave", onLeave);
    };
  }, []);

  return <canvas ref={canvas} className="maskfield" role="img" aria-label="The Clickhand mask, drawn in points and lit red, with lines of light running into its eyes" />;
}
