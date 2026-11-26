"""Record what the original sessions.py and reports.py do, for the Rust port to replay.

Run from the repo root with the Python original on the path and a pinned zone:
    TZ=Europe/Paris PYTHONPATH=reference python3 rust/record/sessions_reports.py
Writes crates/core/tests/cases/{reports.db, sessions_reports.json}. The data is synthetic
(no personal sessions), seeded, and spans DST changes, midnight crossings and
accidental sub-5 s runs, so the cases pin down the original's edge behaviour.
"""
import json
import random
import sqlite3
from datetime import date, datetime, timedelta
from pathlib import Path

from ultron import reports, sessions

OUT = Path("crates/core/tests/cases")
DB = OUT / "reports.db"
LABELS = ["study", "deep work", "rust port", "gym", "untitled", "lecture", "écriture", "日本語"]
rng = random.Random(1337)


def build_db():
    if DB.exists():
        DB.unlink()
    conn = sessions.connect(DB)
    conn.execute("PRAGMA journal_mode=DELETE")  # one file, no -wal next to it
    # 2025-09-01 .. 2026-11-30 covers both 2026 DST changes in Paris (29 Mar, 25 Oct)
    day = date(2025, 9, 1)
    while day <= date(2026, 11, 30):
        for _ in range(rng.choice([0, 0, 1, 1, 2, 3])):
            hour = rng.choice([0, 1, 2, 3, 8, 9, 13, 17, 21, 23])
            start = datetime(day.year, day.month, day.day, hour, rng.randrange(60), rng.randrange(60)).timestamp()
            kind = rng.choice(["timer", "timer", "stopwatch"])
            planned = rng.choice([25 * 60, 50 * 60, 90 * 60]) if kind == "timer" else None
            status = rng.choice(["completed", "completed", "stopped", "interrupted"])
            secs_total = rng.choice([3, 4, 45, 600, 1500, 3000, 5400, 7200, 9000])
            cur = conn.execute(
                "INSERT INTO sessions(kind,label,planned,started,ended,active,status) VALUES (?,?,?,?,?,?,?)",
                (kind, rng.choice(LABELS), planned, start, start + secs_total, secs_total, status))
            sid = cur.lastrowid
            t, left = start, float(secs_total)
            while left > 0:                      # one to three segments with pauses between
                seg = left if rng.random() < 0.5 else max(1.0, round(left * rng.uniform(0.3, 0.7), 3))
                conn.execute("INSERT INTO segments(session_id,start,secs) VALUES (?,?,?)", (sid, t, seg))
                t += seg + rng.choice([0, 30, 600])
                left -= seg
        day += timedelta(days=1)
    # one session still open
    conn.execute("INSERT INTO sessions(kind,label,planned,started,active,status) VALUES ('timer','study',1500,?,600,'open')",
                 (datetime(2026, 11, 26, 10, 0).timestamp(),))
    conn.execute("INSERT INTO segments(session_id,start,secs) VALUES (last_insert_rowid(),?,600)",
                 (datetime(2026, 11, 26, 10, 0).timestamp(),))
    conn.commit()
    return conn


def jsonable(x):
    if isinstance(x, date):
        return x.isoformat()
    if isinstance(x, dict):
        return {k: jsonable(v) for k, v in x.items()}
    if isinstance(x, (list, tuple)):
        return [jsonable(v) for v in x]
    return x


def main():
    conn = build_db()
    cases = {"tz": "Europe/Paris", "fmt": [], "clean_label": [], "bounds": [], "edges": [],
             "summarize": [], "one_liner": [], "streak": [], "day_totals": [], "known_labels": None,
             "relabel_last": None, "tracker": None}

    for s in [0, 0.4, 0.5, 1.5, 2.5, 59, 59.5, 60, 61, 119.6, 3599, 3600, 3725, 86399, 90061, 360000]:
        cases["fmt"].append({"args": [s], "out": reports.fmt(s)})
    for lab in [None, "", "   ", "Deep   Work!!", "  ‘Study’.  ", "\"rust\"", "ÉCRITURE", "a" * 60,
                "Math: chapter 3?", "日本語 study", "tab\tsep\nline", ".,!?", "x" * 39 + "é" * 3]:
        cases["clean_label"].append({"args": [lab], "out": sessions.clean_label(lab)})

    todays = [date(2026, 3, 29), date(2026, 10, 25), date(2026, 1, 1), date(2026, 2, 28),
              date(2026, 11, 26), date(2025, 12, 31), date(2026, 3, 2)]
    for t in todays:
        for p in reports.PERIODS:
            a, b, unit = reports.bounds(p, t)
            cases["bounds"].append({"args": [p, t.isoformat()], "out": [a.isoformat(), b.isoformat(), unit]})
            e, names = reports.edges(a, b, unit)
            cases["edges"].append({"args": [a.isoformat(), b.isoformat(), unit], "out": [e, names]})
            for label in [None, "study", "日本語", "nothing"]:
                s = reports.summarize(conn, p, label=label, today=t)
                cases["summarize"].append({"args": [p, label, t.isoformat()], "out": jsonable(s)})
                cases["one_liner"].append({"args": [p, label, t.isoformat()], "out": reports.one_liner(s)})
        cases["streak"].append({"args": [t.isoformat()], "out": reports.streak(conn, t)})
        first = t - timedelta(days=40)
        cases["day_totals"].append({"args": [first.isoformat(), t.isoformat()],
                                    "out": {d.isoformat(): v for d, v in reports.day_totals(conn, first, t).items()}})
    cases["known_labels"] = reports.known_labels(conn)

    # relabel_last on a copy, so the fixture stays as recorded
    mem = sqlite3.connect(":memory:")
    conn.backup(mem)
    mem.row_factory = sqlite3.Row
    cases["relabel_last"] = {"args": ["  New Label!! "], "out": sessions.relabel_last(mem, "  New Label!! "),
                             "label_after": mem.execute("SELECT label FROM sessions ORDER BY started DESC LIMIT 1").fetchone()[0]}

    # Tracker: a scripted run with fake clocks, then every row it wrote
    clock = {"wall": 1_790_000_000.0, "mono": 1000.0}
    tpath = OUT / "tracker.db"
    if tpath.exists():
        tpath.unlink()
    script = [("start", "timer", "Focus Time!", 1500), ("tick", 40), ("heartbeat",), ("tick", 100), ("pause",),
              ("tick", 300), ("resume",), ("tick", 31), ("heartbeat",), ("tick", 5), ("heartbeat",),
              ("set_planned", 3000), ("tick", 20), ("finish", "completed"), ("tick", 10),
              ("start", "stopwatch", None, None), ("tick", 12), ("start", "timer", "next", 600), ("tick", 7), ("close",)]
    tr = sessions.Tracker(tpath, wall=lambda: clock["wall"], mono=lambda: clock["mono"])
    for op in script:
        if op[0] == "tick":
            clock["wall"] += op[1]; clock["mono"] += op[1]
        elif op[0] == "start":
            tr.start(op[1], op[2], op[3])
        elif op[0] == "finish":
            tr.finish(op[1])
        elif op[0] == "set_planned":
            tr.set_planned(op[1])
        else:
            getattr(tr, op[0])()
    dump = sqlite3.connect(tpath)
    cases["tracker"] = {
        "start": {"wall": 1_790_000_000.0, "mono": 1000.0},
        "script": [list(op) for op in script],
        "sessions": [list(r) for r in dump.execute("SELECT id,kind,label,planned,started,ended,active,status FROM sessions ORDER BY id")],
        "segments": [list(r) for r in dump.execute("SELECT id,session_id,start,secs FROM segments ORDER BY id")],
    }
    dump.close()
    tpath.unlink()

    conn.close()
    (OUT / "sessions_reports.json").write_text(json.dumps(cases, ensure_ascii=False, indent=1))
    n = sum(len(v) for v in cases.values() if isinstance(v, list))
    print(f"recorded {n} cases into {OUT}")


if __name__ == "__main__":
    main()
