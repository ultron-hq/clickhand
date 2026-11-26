import { Eye, MousePointerClick, CheckCheck } from "lucide-react";

/** Hands' loop as a ring of light: look, act, check, and around again. */
export function LoopDiagram() {
  const nodes = [
    { label: "Look", icon: Eye, x: 50, y: 6 },
    { label: "Act", icon: MousePointerClick, x: 88, y: 72 },
    { label: "Check", icon: CheckCheck, x: 12, y: 72 },
  ];
  return (
    <div className="loop" aria-hidden="true">
      <svg viewBox="0 0 400 400" className="loop-ring">
        <defs>
          <linearGradient id="loop-light" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stopColor="#c4302a" stopOpacity="0" />
            <stop offset="0.5" stopColor="#ff7a2f" />
            <stop offset="1" stopColor="#ffcd8c" />
          </linearGradient>
        </defs>
        <circle cx="200" cy="200" r="150" className="loop-track" />
        <g className="loop-spin">
          <circle cx="200" cy="200" r="150" className="loop-arc" stroke="url(#loop-light)" />
          <circle cx="350" cy="200" r="4" className="loop-head" />
        </g>
      </svg>
      <span className="loop-core"><span className="loop-cursor" /></span>
      {nodes.map(({ label, icon: Icon, x, y }) => (
        <span key={label} className="loop-node" style={{ left: `${x}%`, top: `${y}%` }}>
          <Icon size={16} strokeWidth={1.5} />
          {label}
        </span>
      ))}
    </div>
  );
}
