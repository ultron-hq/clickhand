"use client";

import { useEffect, useRef, useState } from "react";

/** One split-flap character: the top half falls to reveal the new value. */
function Flap({ value }: { value: string }) {
  const [shown, setShown] = useState(value);
  const [prev, setPrev] = useState(value);
  const [turn, setTurn] = useState(0);
  const first = useRef(true);

  useEffect(() => {
    if (first.current) {
      first.current = false;
      return;
    }
    if (value === shown) return;
    setPrev(shown);
    setShown(value);
    setTurn((t) => t + 1);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [value]);

  return (
    <span className="flap" aria-hidden="true">
      <span className="flap-half flap-top"><span>{shown}</span></span>
      <span className="flap-half flap-bottom"><span>{turn ? prev : shown}</span></span>
      {turn > 0 && (
        <span key={turn}>
          <span className="flap-half flap-top flap-fold"><span>{prev}</span></span>
          <span className="flap-half flap-bottom flap-unfold"><span>{shown}</span></span>
        </span>
      )}
    </span>
  );
}

export function SplitFlap({ text, label }: { text: string; label: string }) {
  return (
    <span className="splitflap" role="img" aria-label={label}>
      {[...text].map((ch, i) =>
        ch === ":" ? (
          <span key={i} className="flap-colon" aria-hidden="true">:</span>
        ) : (
          <Flap key={i} value={ch} />
        ),
      )}
    </span>
  );
}
