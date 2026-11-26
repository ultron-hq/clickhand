"""Record the original router (brain.py rules and routing) and config modules.

    PYTHONPATH=reference python3 rust/record/router_config.py
Writes:
  crates/core/src/router/skills.json   the skill registry, in the original's order
  crates/core/tests/cases/router_config.json
Skills are stubbed (they echo, or raise Unclear / crash on marker words) and the local
router is absent (ADR-002), so this records the routing logic itself, nothing else.
"""
import json
import os
import tempfile
import types
from pathlib import Path

import tests.test_ultron as corpus
from ultron import brain, live_config, memory, model_settings, preferences
from ultron.skills import Unclear, load_skills

OUT = Path("crates/core/tests/cases")
REG = Path("crates/core/src/router/skills.json")

EXTRA = [
    "", "   ", "START A STOPWATCH", "start a stopwatch for reading", "open spotify and play lofi",
    "no, i meant spotify", "nope it's firefox", "not that, use vlc", "wrong!", "no", "nah try chrome please",
    "what time is it in tokyo", "how much did i work this week", "is it raining?", "remember my locker is 42",
    "who are you", "set a timer for 10 minutes", "volume up", "take a screenshot", "search rust lifetimes",
    "add a task: write the report", "research quantum batteries", "click the blue button", "what's on my screen",
    "open the workspace", "Ultron, open firefox", "café time?", "日本語で検索", "hey", "close this window",
    "move the window to the left", "type hello world", "press ctrl+s", "scroll down", "teach me rust",
    "research how to take a screenshot", "add a task: take a screenshot of the bug",
]


def stub_skills():
    skills = load_skills()
    out = {}
    for name, mod in skills.items():
        def run(text, ctx, _name=name):
            if "UNCLEAR" in text:
                raise Unclear(f"{_name} could not make sense of it")
            if "CRASH" in text:
                raise RuntimeError("boom")
            return f"{_name} ran: {text}"
        out[name] = types.SimpleNamespace(
            NAME=name, MATCH=getattr(mod, "MATCH", None), ONLY_MATCH=getattr(mod, "ONLY_MATCH", False),
            DESCRIPTION=getattr(mod, "DESCRIPTION", ""), USAGE=getattr(mod, "USAGE", None), run=run)
    return skills, out


class FakePlanner:
    """Scripted plans: what the model would have said for these requests."""
    PLANS = {
        "show me some cat videos": {"skill": "web_search", "command": "search youtube for cat videos"},
        "what's the capital of peru": {"answer": "Lima."},
        "put on something relaxing": {"skill": "open_app", "command": "open spotify"},
        "make it UNCLEAR please": {"skill": "open_app", "command": "open UNCLEAR thing"},
        "tell me about my locker": {"answer": "Your locker is 42."},
        "track my focus": {"skill": "time_report", "command": "time report today"},
    }

    def __init__(self):
        self.calls = []

    def plan(self, text, notes=(), cancel=None):
        self.calls.append([text, list(notes)])
        return self.PLANS.get(text)


def record_router():
    real, stubs = stub_skills()
    REG.parent.mkdir(parents=True, exist_ok=True)
    reg = []
    for name, mod in real.items():
        m = getattr(mod, "MATCH", None)
        reg.append({"name": name, "only_match": bool(getattr(mod, "ONLY_MATCH", False)),
                    "pattern": m.pattern if m is not None else None,
                    "ignore_case": bool(m.flags & 2) if m is not None else False,
                    "description": getattr(mod, "DESCRIPTION", ""), "usage": getattr(mod, "USAGE", None)})
    REG.write_text(json.dumps(reg, ensure_ascii=False, indent=1))

    b = brain.Brain.__new__(brain.Brain)
    b.skills = stubs
    texts = list(corpus.ROUTES) + EXTRA
    cases = {"rule": [], "correction": [], "question": [], "route": [], "planner_calls": None, "tables": None}
    for t in texts:
        cases["rule"].append({"args": [t], "out": brain.Brain.rule(b, t)})
        m = brain.CORRECTION.match(t)
        cases["correction"].append({"args": [t], "out": m.group("x") if m else None})
        cases["question"].append({"args": [t], "out": bool(brain.QUESTION.search(t))})

    with tempfile.TemporaryDirectory() as d:
        clock = {"t": 1_790_000_000.0}

        def tick():
            clock["t"] += 1.0
            return clock["t"]

        mem = memory.Memory(Path(d) / "r.db", clock=tick)
        mem.remember("my locker is number 42 at the gym")
        mem.add_alias("my music", "spotify")
        b.memory, b.use_memory, b.use_llm = mem, True, True
        b.planner, b.desktop, b.router, b.last = FakePlanner(), None, None, None
        mono = {"t": 5000.0}
        brain.time.monotonic = lambda: mono["t"]
        script = [
            "open firefox", "open my music", "show me some cat videos", "show me some dog videos",
            "what's the capital of peru", "what's the capital of peru", "where is my locker",
            "tell me about my locker", "put on something relaxing", "put on something relaxing",
            "make it UNCLEAR please", "open UNCLEAR", "volume CRASH", "something nobody planned",
            "new conversation", "track my focus", "start a stopwatch",
            ("last", "open firfox", 10), ("correct", "no, i meant open firefox"),
            ("last", "play sad songs", 10), ("correct", "no i meant what's the capital of peru"),
            ("last", "old thing", 400), ("correct", "no, i meant open vlc"),
            ("correct", "open vlc"),
            "open firfox", "play sad songs",
        ]
        ctx_origin = []
        for step in script:
            if isinstance(step, tuple) and step[0] == "last":
                b.last = (step[1], mono["t"] - step[2])
                cases["route"].append({"op": list(step), "out": None})
                continue
            ctx = brain.Context(dry_run=True)
            if isinstance(step, tuple) and step[0] == "correct":
                res = b.correct(step[1], ctx)
                op = list(step)
            else:
                res = b.route(step, ctx)
                op = ["route", step]
            out = None if res is None else {"reply": res.reply, "skill": res.skill, "conf": res.conf,
                                             "via": res.via, "command": res.command, "ok": res.ok, "origin": ctx.origin}
            cases["route"].append({"op": op, "out": out, "last": list(b.last) if b.last else None})
        cases["planner_calls"] = b.planner.calls
        cases["tables"] = {t: [list(r) for r in mem.conn.execute(f"SELECT * FROM {t} ORDER BY 1")]
                           for t in ("learned", "facts", "aliases")}
        mem.close()
    return cases


def read_file(p):
    """A config file as JSON, or its raw text when it isn't JSON, or None when missing."""
    if not p.exists():
        return None
    try:
        return json.loads(p.read_text())
    except ValueError:
        return {"raw": p.read_text()}


def record_config():
    out = {"preferences": [], "live_config": [], "model_settings": []}
    with tempfile.TemporaryDirectory() as d:
        d = Path(d)
        preferences.DB = d / "ultron.db"
        live_config.CONFIG = d / "gemini.json"
        for k in ("ULTRON_DESKTOP_MODEL", "GEMINI_API_KEY", "GOOGLE_API_KEY"):
            os.environ.pop(k, None)

        def pref(op, *a, **kw):
            try:
                res = {"ok": getattr(preferences, op)(*a, **kw)}
            except ValueError as e:
                res = {"err": str(e)}
            out["preferences"].append({"op": [op, kw], "out": res, "file": read_file(d / "preferences.json")})

        pref("read")
        (d / "preferences.json").write_text("not json")
        pref("read")
        (d / "preferences.json").write_text(json.dumps({"model": "opus", "junk": 1, "effort": "high"}))
        pref("read")
        (d / "preferences.json").write_text(json.dumps(["a", "list"]))
        pref("read")
        (d / "preferences.json").unlink()
        for kw in [{"model": "haiku"}, {"model": "gpt"}, {"effort": "max"}, {"routing_model": "opus"},
                   {"persistent_cursor": 1}, {"persistent_cursor": True}, {"voice_provider": "local"},
                   {"voice_provider": "x"}, {"voice_language": "ar"}, {"voice_language": "fr"},
                   {"voice_auto_send": "yes"}, {"voice_auto_send": False}, {"unknown": 1}, {"effort": "medium", "model": "opus"}]:
            pref("save", **kw)

        def live(op, *a, env=None, **kw):
            os.environ.pop("GEMINI_API_KEY", None); os.environ.pop("GOOGLE_API_KEY", None)
            for k, v in (env or {}).items():
                os.environ[k] = v
            try:
                res = {"ok": getattr(live_config, op)(*a, **kw)}
            except ValueError as e:
                res = {"err": str(e)}
            p = d / "gemini.json"
            out["live_config"].append({"op": [op, list(a), kw, env], "out": res, "file": read_file(p)})

        live("read")
        live("save", "  key-123  ")
        live("read")
        live("read", env={"GOOGLE_API_KEY": "g-key"})
        live("read", env={"GEMINI_API_KEY": "gem", "GOOGLE_API_KEY": "g-key"})
        live("save", "bad key")
        live("save", "")
        live("save", "k", model="gemini-3.8-live/v2.preview_x")
        live("save", "k", model="bad model!")
        live("save", "k", model="m" * 121)
        live("save", "k", voice="Bob")
        live("save", "k2", model="  gemini-x  ", voice="Puck")
        live("read")
        (d / "gemini.json").write_text("[1,2]")
        live("read")
        live("save", "k3")

        os.environ.pop("GEMINI_API_KEY", None); os.environ.pop("GOOGLE_API_KEY", None)
        (d / "preferences.json").unlink(missing_ok=True)
        ms = out["model_settings"]
        for t in ["/model", " /MODEL sonnet", "/settings", "hello", "", "/models", "/speed thorough please"]:
            ms.append({"fn": "is_command", "args": [t], "out": model_settings.is_command(t)})
        for t in ["/settings", "/settings now", "/model", "/model opus", "/model router haiku", "/model desktop sonnet",
                  "/model router", "/model gpt", "/model opus haiku", "/effort", "/effort high", "/effort max",
                  "/speed", "/speed fast", "/speed thorough", "/speed warp", "/MODEL  Haiku", "/unknown"]:
            ms.append({"fn": "handle", "args": [t, True], "out": model_settings.handle(t, dry_run=True)})
        for t in ["/model opus", "/effort medium", "/speed balanced", "/model router sonnet", "/speed fast"]:
            ms.append({"fn": "handle", "args": [t, False], "out": model_settings.handle(t)})
        preferences.save(voice_provider="local")
        ms.append({"fn": "status", "args": [], "out": model_settings.status()})
        os.environ["ULTRON_DESKTOP_MODEL"] = "opus"
        ms.append({"fn": "status", "args": [], "env": {"ULTRON_DESKTOP_MODEL": "opus"}, "out": model_settings.status()})
        os.environ.pop("ULTRON_DESKTOP_MODEL")
        for m, e in [("haiku", "high"), ("sonnet", "low"), ("opus", "medium")]:
            ms.append({"fn": "claude_options", "args": [m, e], "out": model_settings.claude_options(m, e)})
    return out


def main():
    data = {"router": record_router(), "config": record_config()}
    (OUT / "router_config.json").write_text(json.dumps(data, ensure_ascii=False, indent=1))
    r, c = data["router"], data["config"]
    n = sum(len(r[k]) for k in ("rule", "correction", "question", "route")) + sum(len(v) for v in c.values())
    print(f"recorded {n} cases; registry: {len(json.loads(REG.read_text()))} skills")


if __name__ == "__main__":
    main()
