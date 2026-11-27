"use client";

import { useEffect, useRef, useState } from "react";
import { setStopped } from "./stop";

// pixelfx "scifi/emp-blast" in Clickhand's ember palette, baked to a sprite sheet
// (pixelfx by Patch, MIT, https://github.com/patch-gg/pixelfx).
const SHEET = { src: "/fx/emp-blast.png", w: 152, h: 116, cols: 9, frames: 97, fps: 30, pivot: [76, 84] as const };
// Big on desktop, still inside the screen on a phone.
const scaleFor = (vw: number) => (vw < 640 ? Math.max(2, Math.floor((vw / SHEET.w) * 10) / 10) : 5);

/** A machined emergency stop. Pressing it fires an EMP and really freezes everything moving on this page. */
export function KillSwitch() {
  const [down, setDown] = useState(false);
  const fx = useRef<HTMLCanvasElement>(null);
  const sheet = useRef<HTMLImageElement | null>(null);
  const raf = useRef(0);
  const [scale, setScale] = useState(5);

  useEffect(() => {
    const img = new Image();
    img.src = SHEET.src;
    sheet.current = img;
    const fit = () => setScale(scaleFor(window.innerWidth));
    fit();
    addEventListener("resize", fit);
    return () => { cancelAnimationFrame(raf.current); removeEventListener("resize", fit); };
  }, []);

  // Plays on its own clock: the EMP is the one thing still moving once the page is stopped.
  const blast = () => {
    const cv = fx.current, img = sheet.current;
    if (!cv || !img?.complete || matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const ctx = cv.getContext("2d")!;
    cancelAnimationFrame(raf.current);
    const start = performance.now();
    const step = (now: number) => {
      const f = Math.floor(((now - start) / 1000) * SHEET.fps);
      ctx.imageSmoothingEnabled = false;
      ctx.clearRect(0, 0, cv.width, cv.height);
      if (f >= SHEET.frames) return;
      const sx = (f % SHEET.cols) * SHEET.w, sy = Math.floor(f / SHEET.cols) * SHEET.h;
      ctx.drawImage(img, sx, sy, SHEET.w, SHEET.h, 0, 0, cv.width, cv.height);
      raf.current = requestAnimationFrame(step);
    };
    raf.current = requestAnimationFrame(step);
  };

  const toggle = () => {
    const next = !down;
    setDown(next);
    setStopped(next);
    if (next) blast();
  };

  return (
    <div className="estop">
      <div className="estop-plinth">
        <button
          type="button"
          className={`estop-cap${down ? " estop-down" : ""}`}
          aria-pressed={down}
          onClick={toggle}
        >
          <span className="sr">{down ? "Release the stop and let the page move again" : "Stop everything moving on this page"}</span>
        </button>
        <canvas
          ref={fx}
          className="estop-fx"
          width={Math.round(SHEET.w * scale)}
          height={Math.round(SHEET.h * scale)}
          style={{
            // put the effect's ground pivot on the middle of the button
            marginLeft: -SHEET.pivot[0] * scale,
            marginTop: -SHEET.pivot[1] * scale,
          }}
          aria-hidden="true"
        />
      </div>
      <p className="estop-state" aria-live="polite">
        {down ? "Stopped. Every animation on this page is frozen. Press again to release" : "Press it. Everything moving on this page stops"}
      </p>
    </div>
  );
}
