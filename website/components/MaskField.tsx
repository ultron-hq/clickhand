"use client";

import { useEffect, useRef } from "react";

type Dot = {
  tx: number; ty: number;   // home position in the mask (0..1)
  x: number; y: number;     // current position (px)
  vx: number; vy: number;
  light: number;            // 0..1 brightness from the logo
  eye: boolean;
  delay: number;            // when it starts flying home (0..1 of the intro)
  seed: number;
};

const GRID = 96;            // sample resolution of the mask
const INTRO_MS = 2200;
const ease = (t: number) => 1 - Math.pow(1 - t, 4);

/**
 * The Ultron mask drawn as a field of dots. On load they drift in from scattered
 * dust and lock into the mask; the red eyes then follow the pointer, and the dots
 * part around it. Static when reduced motion is on.
 */
export function MaskField() {
  const canvas = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const cv = canvas.current!;
    const ctx = cv.getContext("2d")!;
    const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
    let dots: Dot[] = [];
    let w = 0, h = 0, dpr = 1, size = 0, ox = 0, oy = 0;
    let start = 0, raf = 0, visible = true;
    const pointer = { x: -9999, y: -9999, active: false };
    const gaze = { x: 0, y: 0 };

    const resize = () => {
      dpr = Math.min(devicePixelRatio || 1, 2);
      const r = cv.getBoundingClientRect();
      w = r.width; h = r.height;
      cv.width = Math.round(w * dpr); cv.height = Math.round(h * dpr);
      size = Math.min(w, h) * 0.92;
      ox = (w - size) / 2; oy = (h - size) / 2;
    };

    const img = new Image();
    img.src = "/logo.svg";
    img.onload = () => {
      const s = document.createElement("canvas");
      s.width = GRID; s.height = GRID;
      const sc = s.getContext("2d", { willReadFrequently: true })!;
      sc.drawImage(img, 0, 0, GRID, GRID);
      const px = sc.getImageData(0, 0, GRID, GRID).data;
      resize();
      const next: Dot[] = [];
      for (let y = 0; y < GRID; y++) {
        for (let x = 0; x < GRID; x++) {
          const i = (y * GRID + x) * 4;
          const a = px[i + 3] / 255;
          if (a < 0.35) continue;
          const [r, g, b] = [px[i], px[i + 1], px[i + 2]];
          const eye = r > 170 && g < 140 && b < 140;
          const light = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
          const angle = Math.random() * Math.PI * 2;
          const dist = (0.6 + Math.random() * 0.9) * Math.max(w, h);
          next.push({
            tx: (x + 0.5) / GRID, ty: (y + 0.5) / GRID,
            x: w / 2 + Math.cos(angle) * dist, y: h / 2 + Math.sin(angle) * dist,
            vx: 0, vy: 0, light, eye,
            // the eyes arrive last, so they "open"
            delay: eye ? 0.55 + Math.random() * 0.1 : Math.random() * 0.45,
            seed: Math.random(),
          });
        }
      }
      dots = next;
      start = performance.now();
      if (reduced) draw(performance.now() + INTRO_MS * 2, true);
      else raf = requestAnimationFrame(loop);
    };

    const draw = (now: number, still = false) => {
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, w, h);
      const t = Math.min(1, (now - start) / INTRO_MS);
      const cell = size / GRID;

      // the eyes look toward the pointer, a little
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

      for (const d of dots) {
        let hx = ox + d.tx * size, hy = oy + d.ty * size;
        if (d.eye) { hx += gaze.x; hy += gaze.y; }

        if (still) { d.x = hx; d.y = hy; }
        else {
          const local = Math.max(0, Math.min(1, (t - d.delay) / (1 - d.delay + 0.0001) * 1.6));
          if (local < 1) {
            // intro: glide home on an eased path
            const e = ease(local);
            d.x += (hx - d.x) * (0.02 + e * 0.12);
            d.y += (hy - d.y) * (0.02 + e * 0.12);
          } else {
            // settled: a spring home, pushed away by the pointer
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

        // a few cells shimmer slowly so the face feels alive
        const shimmer = still ? 0 : Math.sin(now * 0.0012 + d.seed * 40) * 0.06;
        const l = Math.max(0, Math.min(1, d.light + shimmer));
        const r = Math.max(0.5, cell * (0.22 + l * 0.3));
        if (d.eye) {
          const on = still ? 1 : Math.min(1, Math.max(0, (t - 0.7) / 0.3));
          ctx.fillStyle = `rgba(255, 81, 77, ${0.25 + on * 0.75})`;
          ctx.fillRect(d.x - r, d.y - r, r * 2, r * 2);
        } else {
          const g = Math.round(58 + l * 170);
          ctx.fillStyle = `rgb(${g}, ${g + 3}, ${g + 8})`;
          ctx.fillRect(d.x - r, d.y - r, r * 2, r * 2);
        }
      }
    };

    const loop = (now: number) => {
      if (visible) draw(now);
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
    const ro = new ResizeObserver(() => {
      const before = { ox, oy, size };
      resize();
      if (reduced && dots.length) draw(performance.now(), true);
      void before;
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

  return <canvas ref={canvas} className="maskfield" role="img" aria-label="The Ultron mask, drawn in points, with red eyes that follow your pointer" />;
}
