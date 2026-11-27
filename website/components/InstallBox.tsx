"use client";

import { useEffect, useState } from "react";
import { Check, Copy } from "lucide-react";

const CMDS = {
  mac: {
    label: "macOS",
    cmd: "curl -fsSL https://clickhand.vercel.app/mac.sh | sh",
    note: "Installs the Clickhand app into Applications and opens it. Its welcome screen connects Claude Code, Codex, Cursor and the rest, and asks macOS for permission once",
  },
  win: {
    label: "Windows",
    cmd: "irm https://clickhand.vercel.app/install.ps1 | iex",
    note: "Then a short setup connects Claude Code, Codex, Cursor, VS Code, Windsurf, Claude Desktop and Gemini CLI",
  },
  linux: {
    label: "Linux",
    cmd: "curl -fsSL https://clickhand.vercel.app/install.sh | sh",
    note: "Then a short setup connects Claude Code, Codex, Cursor, VS Code, Windsurf, Claude Desktop and Gemini CLI",
  },
} as const;

type Os = keyof typeof CMDS;

/** The one-line install, preset to the visitor's OS, with a copy button. */
export function InstallBox() {
  const [os, setOs] = useState<Os>("mac");
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    const ua = navigator.userAgent;
    if (/Windows/i.test(ua)) setOs("win");
    else if (/Linux|X11|CrOS/i.test(ua) && !/Android/i.test(ua)) setOs("linux");
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
      <p className="install-note">{CMDS[os].note}</p>
    </div>
  );
}
