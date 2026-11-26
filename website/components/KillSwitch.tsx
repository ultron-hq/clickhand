"use client";

import { useState } from "react";
import { setStopped } from "./stop";

/** A machined emergency stop. Pressing it really freezes everything moving on this page. */
export function KillSwitch() {
  const [down, setDown] = useState(false);
  const toggle = () => {
    const next = !down;
    setDown(next);
    setStopped(next);
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
      </div>
      <p className="estop-state" aria-live="polite">
        {down ? "Stopped. Every animation on this page is frozen. Press again to release" : "Press it. Everything moving on this page stops"}
      </p>
    </div>
  );
}
