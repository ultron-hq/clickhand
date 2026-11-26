"use client";

import { useEffect, useState } from "react";
import { SplitFlap } from "./SplitFlap";
import { stopped } from "./stop";

const pad = (n: number) => String(n).padStart(2, "0");

/** The island running a real 25 minute countdown, as it would after "start a 25 minute timer". */
export function LiveTimer() {
  const [left, setLeft] = useState(25 * 60);
  useEffect(() => {
    if (matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const id = setInterval(() => {
      if (!stopped()) setLeft((s) => (s <= 1 ? 25 * 60 : s - 1));
    }, 1000);
    return () => clearInterval(id);
  }, []);
  const text = `${pad(Math.floor(left / 60))}:${pad(left % 60)}`;
  return (
    <div className="island island-still">
      <SplitFlap text={text} label={`Timer, ${text} left`} />
      <span className="island-say">Focus timer</span>
    </div>
  );
}
