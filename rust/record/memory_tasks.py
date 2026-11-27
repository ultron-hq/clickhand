"""Record what the original memory.py and tasks.py do, for the Rust port to replay.

    PYTHONPATH=reference python3 rust/record/memory_tasks.py
Writes crates/core/tests/cases/memory_tasks.json. Clocks are faked (a counter), so
every timestamp and every tie in ordering is reproducible.
"""
import json
import sqlite3
import tempfile
from pathlib import Path

from ultron import memory, tasks, sessions

OUT = Path("crates/core/tests/cases")

TEXTS = [
    "", "   ", "Hey Ultron, please open my notes!", "ok ultron: pls play lofi", "yo ultron - what's up?",
    "ULTRON open firefox", "Please please search youtube for cat videos", "Ultronic devices are cool",
    "what’s my router ip?", "“quoted” words … and ellipsis", "set volume to 40%", "email me@site.com now",
    "go to https://example.com/path?q=1#frag", "c++ and c# rock", "tab\tand\nnewline", "café crème brûlée",
    "日本語のテキスト", "mixed: Ünïcödé + ASCII", "trailing dots...", "'single quotes'", "100% sure!!",
    "remind me at 10:30", "a/b/c-d_e", "hey ok ultron yo", "  pls   pls  ", "!!!???", "don't stop",
]
TEMPLATE_PAIRS = [
    ("show me some cat videos", "search youtube for cat videos"),
    ("show me some dog videos please", "search youtube for dog videos"),
    ("open my notes", "open obsidian"),
    ("play some lofi beats on spotify", "spotify play lofi beats"),
    ("hey ultron, search the web for rust borrow checker", "web_search rust borrow checker"),
    ("the cat", "search cat"),
    ("start a timer for 25 minutes for study", "timer 25 min study"),
    ("translate hello world to french", "translate hello world french"),
    ("set volume to 40%", "volume 40%"),
    ("open the github page for ultron", "open https://github.com/ultron-hq/openhud"),
    ("find files named report.pdf", "find report.pdf"),
    ("a b c", "a b c"),
    ("download the video at youtube.com/watch", "yt-dlp youtube.com/watch"),
    ("send email to bob about lunch", "mail bob lunch"),
    ("tell me a joke about cats", "joke cats"),
    ("search for c++ tutorials", "web_search c++ tutorials"),
    ("look up café menus", "web_search café menus"),
    ("open the read-me file for ultron", "open readme ultron"),
    ("check the node.js version for ultron", "node --version ultron"),
    ("show me some cat videos on node.js", "search youtube for cat videos"),
]


def run_memory():
    clock = {"t": 1_790_000_000.0}

    def tick():
        clock["t"] += 1.0
        return clock["t"]

    out = {"normalize": [], "words": [], "build_template": [], "ops": [], "tables": {}}
    for t in TEXTS:
        out["normalize"].append({"args": [t], "out": memory.normalize(t)})
        out["words"].append({"args": [t], "out": memory.words(t)})
    for r, c in TEMPLATE_PAIRS:
        res = memory.build_template(r, c)
        out["build_template"].append({"args": [r, c], "out": list(res) if res else None})

    with tempfile.TemporaryDirectory() as d:
        m = memory.Memory(Path(d) / "m.db", clock=tick)
        script = [
            ("remember", "My router IP is 192.168.1.1."), ("remember", "my router ip is 192.168.1.1"),
            ("remember", "The wifi password is hunter2"), ("remember", "my api key is sk-123"),
            ("remember", "   "), ("remember", "Mom's birthday is on March 3rd"),
            ("remember", "I park on level 2 at the office garage"), ("remember", "Dentist appointments are on Tuesdays"),
            ("facts_about", "what is my router ip", 5), ("facts_about", "birthday", 5), ("facts_about", "", 3),
            ("recall", "what's my router ip"), ("recall", "where did i park"), ("recall", "when is mom's birthday"),
            ("recall", "router"), ("recall", "the"), ("recall", "what is the weather"),
            ("add_alias", "my notes", "obsidian."), ("add_alias", "Hey Ultron, my music", "spotify"),
            ("add_alias", "obsidian", "Obsidian"), ("add_alias", "", "x"), ("add_alias", "my notes", "logseq"),
            ("add_alias", "work stuff", "the work folder"),
            ("expand", "open my notes"), ("expand", "Open My Notes and my music"), ("expand", "open my notesy"),
            ("expand", "open o'my notes"), ("expand", "check work   stuff today"),
            ("learn", "show me some cat videos", "search", "search youtube for cat videos", None, "haiku"),
            ("learn", "what is the capital of france", "llm", None, "Paris.", "haiku"),
            ("learn", "open my editor", "apps", "open code", None, "correction"),
            ("learn", "open my editor", "apps", "open vim", None, "haiku"),
            ("learn", "play some lofi beats on spotify", "media", "spotify play lofi beats", None, "haiku"),
            ("learn", "", "apps", "x", None, "haiku"),
            ("learn", "open the read-me file for ultron", "apps", "open readme ultron", None, "haiku"),
            ("lookup", "open the read-me file for jarvis"),
            ("lookup", "show me some dog videos"), ("lookup", "Show me some cat videos!"),
            ("lookup", "show me some a b c d e f g h videos"), ("lookup", "what is the capital of france"),
            ("lookup", "open my editor"), ("lookup", "play some jazz on spotify"), ("lookup", "nothing learned"),
            ("used", "@last_lookup"), ("failed", "@last_lookup"),
            ("tick", 31 * 86400), ("lookup", "what is the capital of france"),
            ("counts",), ("log_request", "open firefox", "apps", 12.5), ("log_request", "x" * 400, "llm", None),
            ("routes", 7),
            ("set_spot", "Buy Half", 1520.7, 640), ("set_spot", "buy half", 10, 20), ("set_spot", "Hey Ultron, sell", 5, 6),
            ("get_spot", "BUY HALF"), ("get_spot", "missing"), ("spots",), ("forget_spot", "sell"), ("forget_spot", "sell"),
            ("facts",), ("aliases",),
            ("forget", "that"), ("forget", "router"), ("forget", "my music"), ("forget", "open my editor"),
            ("forget", "the last one"), ("forget", "  .!? "), ("forget", "dentist appointments"),
            ("counts",), ("forget_everything",), ("counts",),
        ]
        last_id = None
        for op in script:
            name, args = op[0], list(op[1:])
            if name == "tick":
                clock["t"] += args[0]
                out["ops"].append({"op": op, "out": None})
                continue
            if args and args[0] == "@last_lookup":
                args = [last_id]
            res = getattr(m, name)(*args)
            if name == "lookup" and res:
                last_id = res["id"]
            if isinstance(res, tuple):
                res = list(res)
            if name in ("facts_about", "spots", "aliases"):
                res = [list(x) for x in res]
            out["ops"].append({"op": [name, *args], "out": res, "last_change": list(m.last_change) if m.last_change else None})
        for table, cols in [("facts", "id,text,norm,created"), ("aliases", "id,phrase,meaning,created"),
                            ("learned", "id,kind,key,skill,command,answer,source,example,uses,fails,created,last_used"),
                            ("spots", "name,x,y,created"), ("requests", "id,at,text,route,ms")]:
            out["tables"][table] = [list(r) for r in m.conn.execute(f"SELECT {cols} FROM {table} ORDER BY 1")]
        m.close()
    return out


def run_tasks():
    counter = {"t": 1_790_000_000.0}

    def fake_time():
        counter["t"] += 1.0
        return counter["t"]

    tasks.time.time = fake_time
    out = {"validate": [], "ops": []}
    for fields, create in [
        ({"title": "  Write report  "}, True), ({}, True), ({"title": ""}, True), ({"title": "   "}, True),
        ({"title": "x" * 240}, True), ({"title": "x" * 241}, True), ({"title": 5}, True), ({"nope": 1}, False),
        ({"title": "ok", "status": "doing"}, False), ({"status": "later"}, False), ({"priority": "urgent"}, False),
        ({"priority": "high"}, False), ({"due": ""}, False), ({"due": None}, False), ({"due": 0}, False),
        ({"due": "2026-12-01"}, False), ({"due": "20261201"}, False), ({"due": "2026-W49-2"}, False),
        ({"due": "2026W492"}, False), ({"due": "2026-W49"}, False), ({"due": "2026W53"}, False), ({"due": "2026-1-5"}, False), ({"due": "2026-02-30"}, False),
        ({"due": "tomorrow"}, False), ({"due": 20261201}, False), ({"status": None}, False),
        ({"title": "日本語のタスク", "due": "2027-01-01", "priority": "low"}, True),
    ]:
        try:
            res = {"ok": tasks.validate(fields, create)}
        except ValueError as e:
            res = {"err": str(e)}
        out["validate"].append({"args": [fields, create], "out": res})

    with tempfile.TemporaryDirectory() as d:
        conn = sessions.connect(Path(d) / "t.db")
        conn.row_factory = sqlite3.Row
        # two sessions to link to tasks
        for label, active in [("write report", 1500.0), ("study", 3000.0), ("write report", 900.0)]:
            conn.execute("INSERT INTO sessions(kind,label,planned,started,active,status) VALUES ('timer',?,1500,1790000000,?, 'completed')", (label, active))
        conn.commit()
        script = [
            ("create", {"title": "Write report", "priority": "high", "due": "2026-12-01"}),
            ("create", {"title": "Study rust", "status": "doing"}),
            ("create", {"title": "Groceries", "priority": "low"}),
            ("create", {"title": "Ship site", "status": "done"}),
            ("create", {"title": "Plan week", "due": "2026-11-30"}),
            ("create", {"title": "bad", "status": "nope"}),
            ("link", 1, 1), ("link", 2, 2), ("link", 1, 3), ("link", 2, 1),
            ("update", 3, {"status": "done"}), ("update", 4, {"status": "todo", "title": "Ship site v2"}),
            ("update", 1, {"due": None}), ("update", 99, {"title": "ghost"}), ("update", 2, {"weird": 1}),
            ("get", 5), ("get", 42),
            ("listing",),
        ]
        for op in script:
            name, args = op[0], list(op[1:])
            try:
                if name == "link":
                    tasks.ensure(conn)
                    try:
                        conn.execute("INSERT INTO task_sessions(session_id,task_id) VALUES(?,?)", (args[1], args[0]))
                        conn.commit()
                        res = "linked"
                    except sqlite3.IntegrityError:
                        conn.rollback()
                        res = "already linked"
                else:
                    res = getattr(tasks, name)(conn, *args)
                out["ops"].append({"op": op, "out": {"ok": res}})
            except ValueError as e:
                out["ops"].append({"op": op, "out": {"err": str(e)}})
        conn.close()
    return out


def main():
    data = {"memory": run_memory(), "tasks": run_tasks()}
    (OUT / "memory_tasks.json").write_text(json.dumps(data, ensure_ascii=False, indent=1))
    m, t = data["memory"], data["tasks"]
    n = len(m["normalize"]) + len(m["words"]) + len(m["build_template"]) + len(m["ops"]) + len(t["validate"]) + len(t["ops"])
    print(f"recorded {n} cases")


if __name__ == "__main__":
    main()
