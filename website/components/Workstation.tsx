"use client";

import { useEffect, useRef, useState } from "react";
import { SplitFlap } from "./SplitFlap";
import { pausableWait } from "./stop";

type Line =
  | { kind: "user"; text: string }
  | { kind: "tool"; name: string; args: string }
  | { kind: "result"; text: string }
  | { kind: "say"; text: string };

type App = "settings" | "expense";

const FEATURES = [
  { id: "wiki", label: "Wikis", note: "Host documentation for this repository" },
  { id: "issues", label: "Issues", note: "Track bugs, tasks and ideas" },
  { id: "discussions", label: "Discussions", note: "A place for questions and open-ended talk" },
  { id: "projects", label: "Projects", note: "Plan work on boards and tables" },
];

const INVOICE = { vendor: "Atlas Print Co.", amount: "412.80", date: "2026-11-03" };

const pad = (n: number) => String(n).padStart(2, "0");

export function Workstation() {
  const screen = useRef<HTMLDivElement>(null);
  const targets = useRef<Record<string, HTMLElement | null>>({});
  const [lines, setLines] = useState<Line[]>([]);
  const [typing, setTyping] = useState("");
  const [app, setApp] = useState<App>("settings");
  const [on, setOn] = useState<Record<string, boolean>>({ issues: true, discussions: false, wiki: false, projects: true });
  const [form, setForm] = useState({ vendor: "", amount: "", date: "", sent: false, focus: "" });
  const [ring, setRing] = useState({ x: 0.72, y: 0.82, press: 0 });
  const [island, setIsland] = useState("");
  const [clock, setClock] = useState("--:--");
  const [reading, setReading] = useState(false);

  // The island shows the real local time.
  useEffect(() => {
    const tick = () => {
      const d = new Date();
      setClock(`${pad(d.getHours())}:${pad(d.getMinutes())}`);
    };
    tick();
    const id = setInterval(tick, 1000);
    return () => clearInterval(id);
  }, []);

  useEffect(() => {
    const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduced) {
      // Show the finished first task, still.
      setLines([
        { kind: "user", text: "turn on Discussions in the repo settings" },
        { kind: "tool", name: "hands.screenshot", args: "" },
        { kind: "tool", name: "hands.click", args: "1184, 452" },
        { kind: "result", text: "Discussions is checked" },
        { kind: "say", text: "Done. Discussions is on for this repository" },
      ]);
      setOn((o) => ({ ...o, discussions: true }));
      return;
    }

    let alive = true;
    const wait = (ms: number) => pausableWait(ms, () => alive);
    const push = (l: Line) => setLines((ls) => [...ls, l].slice(-9));

    const point = async (id: string) => {
      const el = targets.current[id];
      const box = screen.current?.getBoundingClientRect();
      if (!el || !box) return "0, 0";
      const r = el.getBoundingClientRect();
      const x = (r.left + Math.min(r.width / 2, 18) - box.left) / box.width;
      const y = (r.top + r.height / 2 - box.top) / box.height;
      setRing((p) => ({ ...p, x, y }));
      await wait(820);
      return `${Math.round(x * 1920)}, ${Math.round(y * 1080)}`;
    };
    const press = async () => {
      setRing((p) => ({ ...p, press: p.press + 1 }));
      await wait(220);
    };
    const ask = async (text: string) => {
      for (let i = 1; i <= text.length; i++) {
        setTyping(text.slice(0, i));
        await wait(28);
      }
      await wait(260);
      setTyping("");
      push({ kind: "user", text });
      await wait(500);
    };
    const look = async () => {
      push({ kind: "tool", name: "hands.screenshot", args: "" });
      await wait(650);
    };
    const typeInto = async (field: "vendor" | "amount" | "date", value: string) => {
      const at = await point(field);
      await press();
      push({ kind: "tool", name: "hands.click", args: at });
      setForm((f) => ({ ...f, focus: field }));
      push({ kind: "tool", name: "hands.type", args: `"${value}"` });
      for (let i = 1; i <= value.length; i++) {
        setForm((f) => ({ ...f, [field]: value.slice(0, i) }));
        await wait(38);
      }
      await wait(250);
    };

    (async () => {
      try {
        await wait(900);
        while (alive) {
          // Task 1: a settings toggle
          setLines([]);
          setApp("settings");
          setOn({ issues: true, discussions: false, wiki: false, projects: true });
          await ask("turn on Discussions in the repo settings");
          await look();
          const at = await point("discussions");
          await press();
          push({ kind: "tool", name: "hands.click", args: at });
          setOn((o) => ({ ...o, discussions: true }));
          await wait(400);
          await look();
          push({ kind: "result", text: "Discussions is checked" });
          await wait(500);
          push({ kind: "say", text: "Done. Discussions is on for this repository" });
          setIsland("Discussions is on");
          await wait(2600);
          setIsland("");

          // Task 2: a form filled from a file
          setLines([]);
          setForm({ vendor: "", amount: "", date: "", sent: false, focus: "" });
          setApp("expense");
          await ask("fill the expense form from invoice-nov.pdf");
          push({ kind: "tool", name: "read_file", args: "invoice-nov.pdf" });
          setReading(true);
          await wait(900);
          setReading(false);
          push({ kind: "result", text: `${INVOICE.vendor}, €${INVOICE.amount}, 3 Nov 2026` });
          await wait(500);
          await look();
          await typeInto("vendor", INVOICE.vendor);
          await typeInto("amount", INVOICE.amount);
          await typeInto("date", INVOICE.date);
          const sendAt = await point("submit");
          await press();
          push({ kind: "tool", name: "hands.click", args: sendAt });
          setForm((f) => ({ ...f, sent: true, focus: "" }));
          await wait(500);
          await look();
          push({ kind: "result", text: "The form shows “Expense submitted”" });
          await wait(400);
          push({ kind: "say", text: "Submitted €412.80 to Atlas Print Co." });
          setIsland("Expense submitted");
          await wait(3000);
          setIsland("");
        }
      } catch {
        /* unmounted */
      }
    })();
    return () => {
      alive = false;
    };
  }, []);

  const ref = (id: string) => (el: HTMLElement | null) => {
    targets.current[id] = el;
  };

  return (
    <div className="rig">
      <div className="bezel">
        <div className="screen" ref={screen}>
          <div className={`island${island ? " island-talk" : ""}`} aria-live="polite">
            <SplitFlap text={clock} label={`The time is ${clock}`} />
            <span className="island-say">{island}</span>
          </div>

          <section className="term" aria-label="Claude Code session driving the desktop">
            <header className="win-bar">
              <span className="dots" aria-hidden="true"><i /><i /><i /></span>
              <span>claude</span>
              <span className="muted">clickhand connected</span>
            </header>
            <div className="term-body">
              {lines.map((l, i) => (
                <p key={i} className={`t-${l.kind}`}>
                  {l.kind === "user" && <><span className="t-caret">&gt;</span>{l.text}</>}
                  {l.kind === "tool" && <><span className="t-dot" aria-hidden="true" /><b>{l.name}</b>({l.args})</>}
                  {l.kind === "result" && <><span className="t-elbow" aria-hidden="true">⎿</span>{l.text}</>}
                  {l.kind === "say" && l.text}
                </p>
              ))}
              <p className="t-input">
                <span className="t-caret">&gt;</span>
                {typing}
                <span className="t-cursor" aria-hidden="true" />
              </p>
            </div>
          </section>

          <section className="appwin" aria-label={app === "settings" ? "Repository settings" : "New expense form"}>
            <header className="win-bar">
              <span className="dots" aria-hidden="true"><i /><i /><i /></span>
              <span>{app === "settings" ? "Repository settings" : "New expense"}</span>
            </header>
            {app === "settings" ? (
              <div className="app-body">
                <h3>Features</h3>
                <ul className="feat">
                  {FEATURES.map((f) => (
                    <li key={f.id}>
                      <span className={`check${on[f.id] ? " check-on" : ""}`} ref={ref(f.id)} aria-hidden="true" />
                      <span>
                        <b>{f.label}</b>
                        <small>{f.note}</small>
                      </span>
                    </li>
                  ))}
                </ul>
              </div>
            ) : (
              <div className="app-body">
                {form.sent ? (
                  <div className="sent">
                    <b>Expense submitted</b>
                    <small>{INVOICE.vendor}, €{INVOICE.amount}, sent for approval</small>
                  </div>
                ) : (
                  <>
                    <label className="fld">
                      <span>Vendor</span>
                      <span className={`input${form.focus === "vendor" ? " input-focus" : ""}`} ref={ref("vendor")}>{form.vendor}</span>
                    </label>
                    <div className="fld-row">
                      <label className="fld">
                        <span>Amount, EUR</span>
                        <span className={`input num${form.focus === "amount" ? " input-focus" : ""}`} ref={ref("amount")}>{form.amount}</span>
                      </label>
                      <label className="fld">
                        <span>Date</span>
                        <span className={`input num${form.focus === "date" ? " input-focus" : ""}`} ref={ref("date")}>{form.date}</span>
                      </label>
                    </div>
                    <span className="submit" ref={ref("submit")}>Submit expense</span>
                  </>
                )}
              </div>
            )}
          </section>

          <section className={`pdf${reading ? " pdf-read" : ""}`} aria-label="invoice-nov.pdf">
            <header className="win-bar">
              <span className="dots" aria-hidden="true"><i /><i /><i /></span>
              <span>invoice-nov.pdf</span>
            </header>
            <div className="paper">
              <div className="paper-head"><b>Atlas Print Co.</b><span>INV-2611</span></div>
              <p className="paper-date">3 November 2026</p>
              <dl>
                <div><dt>Posters, A2, 40 copies</dt><dd>€344.00</dd></div>
                <div><dt>VAT 20%</dt><dd>€68.80</dd></div>
                <div className="paper-total"><dt>Total</dt><dd>€412.80</dd></div>
              </dl>
            </div>
          </section>

          <span
            className="ring"
            style={{ transform: `translate(calc(${ring.x} * 100cqw - 50%), calc(${ring.y} * 100cqh - 50%))` }}
            aria-hidden="true"
          >
            <span key={ring.press} className={ring.press ? "ring-pulse" : undefined} />
          </span>
        </div>
      </div>
    </div>
  );
}
