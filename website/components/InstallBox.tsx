"use client";

import { useEffect, useState } from "react";
import { Check, Copy } from "lucide-react";

const CMDS = {
  unix: { label: "macOS / Linux", cmd: "curl -fsSL https://clickhand.vercel.app/install.sh | sh" },
  win: { label: "Windows", cmd: "irm https://clickhand.vercel.app/install.ps1 | iex" },
} as const;

type Os = keyof typeof CMDS;

/** The one-line install, preset to the visitor's OS, with a copy button. */
export function InstallBox() {
  const [os, setOs] = useState<Os>("unix");
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (/Windows/i.test(navigator.userAgent)) setOs("win");
  }, []);

  async function copy() {
    try {
      await navigator.clipboard.writeText(CMDS[os].cmd);
      setCopied(true);
      setTimeout(() => setCopied(false), 1600);
    } catch {
      /* clipboard blocked: the command is still selectable */
    }
  }

  return (
    <div className="install" id="install">
      <div className="install-tabs" role="tablist" aria-label="Your system">
        {(Object.keys(CMDS) as Os[]).map((k) => (
          <button
            key={k}
            role="tab"
            aria-selected={os === k}
            className={os === k ? "on" : undefined}
            onClick={() => {
              setOs(k);
              setCopied(false);
            }}
          >
            {CMDS[k].label}
          </button>
        ))}
      </div>
      <div className="install-line">
        <code>{CMDS[os].cmd}</code>
        <button className="install-copy" onClick={copy} aria-label={copied ? "Copied" : "Copy the install command"}>
          {copied ? <Check size={16} strokeWidth={2} /> : <Copy size={16} strokeWidth={1.75} />}
        </button>
      </div>
      <p className="install-note">
        Then a short setup connects Claude Code, Codex, Cursor, VS Code, Windsurf, Claude Desktop and Gemini CLI
      </p>
    </div>
  );
}
