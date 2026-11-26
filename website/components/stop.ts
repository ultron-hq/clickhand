// The page's own kill switch: pressing it freezes every moving part, like
// `touch ~/.cache/ultron/hands.stop` freezes Ultron's hands.
declare global {
  interface Window {
    __ultronStopped?: boolean;
  }
}

export const stopped = () => typeof window !== "undefined" && !!window.__ultronStopped;

export function setStopped(on: boolean) {
  window.__ultronStopped = on;
  document.documentElement.classList.toggle("stopped", on);
  window.dispatchEvent(new CustomEvent("ultron:stop", { detail: on }));
}

/** A timeout that does not count time while the page is stopped. */
export function pausableWait(ms: number, alive: () => boolean): Promise<void> {
  return new Promise((resolve, reject) => {
    let left = ms;
    let last = performance.now();
    const step = () => {
      if (!alive()) return reject(new Error("stop"));
      const now = performance.now();
      if (!stopped()) left -= now - last;
      last = now;
      if (left <= 0) resolve();
      else setTimeout(step, Math.min(left, 50));
    };
    step();
  });
}
