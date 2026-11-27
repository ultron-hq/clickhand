"""Record the original commands.py (prepare, suggestions) and Brain.handle.

    PYTHONPATH=reference python3 rust/record/commands.py
Writes crates/core/tests/cases/commands.{db,json}. Generated abilities (wave 6) are a
fixed list here; the database holds a few tasks, installed abilities and runs.
"""
import json
import sqlite3
import tempfile
import types
from pathlib import Path

from ultron import abilities, brain, commands, memory, runs, sessions, tasks
from ultron.skills import Unclear, load_skills

OUT = Path("crates/core/tests/cases")
DB = OUT / "commands.db"
GENERATED = [
    {"id": "g1", "name": "Habit Tracker", "command": "/habits", "usage": "[add|list] <habit>", "description": "Track daily habits"},
    {"id": "g2", "name": "Unit Converter", "command": "/convert", "usage": "", "description": "Convert units"},
]
INPUTS = [
    "", "   ", "/help", "/commands", "/HELP", "/unknown thing", "/habits add read", "/convert", "/model opus", "/settings",
    "/voice", "/today", "/quest finish chapter 3", "/log sleep 7h", "/dua morning", "/ayah", "/ability Unit Converter",
    "/ability nope", "/ability", "/build a pomodoro timer", "/abilities", "/deploy list", "/workspace notes", "/workspace",
    "/hacker", "/hacker --authorized ~/proj", "/agents", "/agents --count 3 compare db engines", "/task", "/task Buy @tasks milk",
    "/focus 8", "/focus #8", "/focus x", "/done 3", "/timer", "/timer 25", "/timer 25 deep work", "/timer 0", "/timer 241",
    "/timer 1000", "/new", "/new now", "/stop", "/report", "/tasks", "/tasks all", "/runs", "/research", "/research rust async",
    "/teach @tasks lifetimes", "/analyze @screen what is this", "/do open firefox", "/paper", "/paper scalp btc",
    "build me something", "make me a habit tracker app", "please create a simple todo app", "make me laugh",
    "create a task to run the command", "can you build me a dashboard for sales", "open Habit Tracker", "open my habit tracker",
    "show my unit converter", "run Hidden Ability", "open firefox", "@tasks what should I do first", "@screen",
    "@task:1 summarize", "@task:999 go", "@task:x go", "what about @task:", "email me@site.com", "@tasks @tasks @activity hi",
    "@screen @tasks @activity @task:1 @task:2 @task:3 too many", "@SCREEN explain", "a@screen nope", "@screen.png file",
    "/research @activity what changed", "plain request", "timer 10 min",
]
SUGGEST = [("/", None), ("/ti", None), ("/model ", None), ("/model r", None), ("/model router o", None), ("/speed f", None),
           ("@", None), ("@ta", None), ("@task:", None), ("hello /ti", None), ("/hab", None), ("plain", None),
           ("/effort h", None), ("/model desktop ", None), ("/ti and more", 3), ("ask @scr", None)]


def build_db():
    if DB.exists():
        DB.unlink()
    conn = sessions.connect(DB)
    conn.execute("PRAGMA journal_mode=DELETE")
    tasks.ensure(conn)
    conn.executescript(abilities.SCHEMA)
    conn.executescript(runs.SCHEMA)
    for i, (title, status) in enumerate([("Write report", "todo"), ("Ship site", "doing"), ("Old thing", "done"), ("Study rust", "todo")], 1):
        conn.execute("INSERT INTO tasks(title,status,priority,created,updated) VALUES (?,?, 'normal', ?, ?)", (title, status, 1790000000 + i, 1790000000 + i))
    cols = [r[1] for r in conn.execute("PRAGMA table_info(abilities)")]
    for name, active in [("Unit Converter", 1), ("Hidden Ability", None), ("Habit Tracker", 1)]:
        vals = {c: None for c in cols}
        vals.update({"id": "a-" + name.split()[0].lower(), "name": name, "active_version": active})
        for c in cols:
            if vals[c] is None and c not in ("active_version",):
                vals[c] = 0 if c in ("enabled", "created", "updated") else ""
        conn.execute(f"INSERT INTO abilities({','.join(cols)}) VALUES ({','.join('?' * len(cols))})", [vals[c] for c in cols])
    rcols = [r[1] for r in conn.execute("PRAGMA table_info(agent_runs)")]
    for i in range(1, 7):
        vals = {c: (0 if c in ("started", "ended", "created", "updated") else "") for c in rcols if c != "id"}
        vals.update({"goal": f"goal {i}", "status": "done" if i % 2 else "failed", "summary": ("summary " * 100)[: 50 + i * 120]})
        keys = list(vals)
        conn.execute(f"INSERT INTO agent_runs({','.join(keys)}) VALUES ({','.join('?' * len(keys))})", [vals[k] for k in keys])
    conn.commit()
    conn.close()


def req(r):
    return {"goal": r.goal, "reply": r.reply, "skill": r.skill, "mode": r.mode, "references": r.references,
            "literal_title": r.literal_title, "timer": list(r.timer) if r.timer else None, "settings_command": r.settings_command}


def main():
    build_db()
    commands.generated_commands = lambda path=None: GENERATED
    out = {"prepare": [], "suggestions": [], "handle": [], "requests": None, "generated": GENERATED}
    for t in INPUTS:
        try:
            out["prepare"].append({"args": [t], "out": {"ok": req(commands.prepare(t, path=DB))}})
        except ValueError as e:
            out["prepare"].append({"args": [t], "out": {"err": str(e)}})
    for text, cursor in SUGGEST:
        span, items = commands.suggestions(text, cursor, path=DB)
        out["suggestions"].append({"args": [text, cursor], "out": [list(span) if span else None, [list(i) for i in items]]})

    # Brain.handle: commands in front of the router, with stub skills and no model
    real = load_skills()
    stubs = {}
    for name, mod in real.items():
        def run(text, ctx, _name=name):
            if "UNCLEAR" in text:
                raise Unclear(f"{_name} could not make sense of it")
            return f"{_name} ran: {text}"
        stubs[name] = types.SimpleNamespace(NAME=name, MATCH=getattr(mod, "MATCH", None),
                                            ONLY_MATCH=getattr(mod, "ONLY_MATCH", False), DESCRIPTION="", run=run)
    with tempfile.TemporaryDirectory() as d:
        clock = {"t": 1_790_000_000.0}

        def tick():
            clock["t"] += 1.0
            return clock["t"]

        b = brain.Brain.__new__(brain.Brain)
        b.skills, b.planner, b.desktop, b.router, b.last = stubs, None, None, None, None
        b.memory = memory.Memory(Path(d) / "h.db", clock=tick)
        brain.time.monotonic = lambda: 7000.0
        brain.time.perf_counter = lambda: 1.0
        original_db = commands.DB
        commands.DB = DB
        import ultron.preferences as prefs
        prefs.DB = Path(d) / "ultron.db"
        import ultron.live_config as live
        live.CONFIG = Path(d) / "gemini.json"   # never the real settings file
        for t in ["/help", "/timer 25 deep work", "/task Buy milk", "/model opus", "/settings", "/stop", "open firefox",
                  "no, i meant open chromium", "/research rust UNCLEAR", "something with no rule", "/unknown", "@tasks plan my day"]:
            ctx = brain.Context(dry_run=True)
            reply, skill, conf = b.handle(t, ctx)
            out["handle"].append({"args": [t], "out": [reply, skill, conf],
                                  "ctx": {"request": ctx.request, "mode": ctx.forced_mode, "literal_title": getattr(ctx, "literal_title", None),
                                          "timer": list(ctx.explicit_timer) if getattr(ctx, "explicit_timer", None) else None,
                                          "references": ctx.references},
                                  "last": list(b.last) if b.last else None})
        commands.DB = original_db
        out["requests"] = [list(r) for r in b.memory.conn.execute("SELECT text, route, ms FROM requests ORDER BY id")]
        b.memory.close()
    (OUT / "commands.json").write_text(json.dumps(out, ensure_ascii=False, indent=1))
    print(f"recorded {len(out['prepare']) + len(out['suggestions']) + len(out['handle'])} cases")


if __name__ == "__main__":
    main()
