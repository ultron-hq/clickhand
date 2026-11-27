"use client";

import { useEffect, useRef } from "react";

/** The real Teach recording, looping in a screen frame under the hero.
 *  Plays only while visible, and not at all for people who prefer reduced motion. */
export function HeroClip() {
  const video = useRef<HTMLVideoElement>(null);

  useEffect(() => {
    const v = video.current;
    if (!v) return;
    const still = matchMedia("(prefers-reduced-motion: reduce)");
    const io = new IntersectionObserver(
      ([e]) => {
        if (e.isIntersecting && !still.matches) v.play().catch(() => {});
        else v.pause();
      },
      { threshold: 0.15 },
    );
    io.observe(v);
    return () => io.disconnect();
  }, []);

  return (
    <figure className="hero-clip">
      <div className="clip-frame">
        <div className="clip-bar" aria-hidden="true">
          <span /><span /><span />
          <p>/clickhand-teach how do I add RSI on TradingView?</p>
        </div>
        <video
          ref={video}
          muted
          loop
          playsInline
          preload="metadata"
          poster="/media/hero-loop.jpg"
          aria-label="Codex teaching how to add the RSI indicator on TradingView: an orange frame shows the AI is watching, and a glowing box with a caption marks each button to click"
        >
          <source src="/media/hero-loop.webm" type="video/webm" />
          <source src="/media/hero-loop.mp4" type="video/mp4" />
        </video>
      </div>
      <figcaption>
        A real recording: Codex teaches RSI on TradingView with on-screen highlights, and the person does the clicks. Sped up 6×
      </figcaption>
    </figure>
  );
}
