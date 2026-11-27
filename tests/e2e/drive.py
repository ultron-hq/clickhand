#!/usr/bin/env python3
"""End-to-end check of an installed ultron-hands, the way an agent uses it.

    python3 tests/e2e/drive.py [path/to/ultron-hands] [out-dir]

Starts the MCP server, opens a small window that writes one typed line to a file,
then uses Hands to find the window, click into it and type. Passes only if the
file holds exactly what was typed. Also checks the kill switch and the CLI.
Screenshots go to out-dir. Standard library only; Linux (X11), macOS, Windows.
"""
import base64
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time

BIN = sys.argv[1] if len(sys.argv) > 1 else (shutil.which("ultron-hands") or "ultron-hands")
OUT = sys.argv[2] if len(sys.argv) > 2 else "e2e-out"
TITLE = "ultron-e2e"
TEXT = "hello from ultron 42"
os.makedirs(OUT, exist_ok=True)

results = []


def check(name, ok, detail=""):
    results.append({"check": name, "ok": bool(ok), "detail": str(detail)[:300]})
    print(("PASS " if ok else "FAIL ") + name + (f": {detail}" if detail else ""), flush=True)
    return ok


class Server:
    def __init__(self):
        self.p = subprocess.Popen([BIN, "serve"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1)
        self.n = 0

    def rpc(self, method, params=None, notify=False):
        msg = {"jsonrpc": "2.0", "method": method}
        if params is not None:
            msg["params"] = params
        if not notify:
            self.n += 1
            msg["id"] = self.n
        self.p.stdin.write(json.dumps(msg) + "\n")
        self.p.stdin.flush()
        if notify:
            return None
        while True:
            line = self.p.stdout.readline()
            if not line:
                raise RuntimeError("server exited")
            r = json.loads(line)
            if r.get("id") == self.n:
                return r

    def call(self, tool, **args):
        r = self.rpc("tools/call", {"name": tool, "arguments": args})["result"]
        text = " ".join(c.get("text", "") for c in r["content"] if c["type"] == "text")
        images = [c["data"] for c in r["content"] if c["type"] == "image"]
        return r.get("isError", False), text, images

    def close(self):
        self.p.stdin.close()
        self.p.wait(timeout=10)


def _cls(u, h):
    import ctypes
    buf = ctypes.create_unicode_buffer(64)
    u.GetClassNameW(h, buf, 64)
    return buf.value


shots = 0


def save(images, label):
    global shots
    for data in images:
        shots += 1
        with open(os.path.join(OUT, f"{shots:02d}-{label}.png"), "wb") as f:
            f.write(base64.b64decode(data))


def reader_window(out_file):
    """A window titled TITLE that writes the first line typed into it to out_file."""
    if sys.platform == "win32":
        ps = f"$host.UI.RawUI.WindowTitle='{TITLE}'; $l = Read-Host 'type here'; Set-Content -NoNewline -Path '{out_file}' -Value $l"
        return subprocess.Popen(["powershell", "-NoProfile", "-Command", ps], creationflags=subprocess.CREATE_NEW_CONSOLE)
    if sys.platform == "darwin":
        script = os.path.join(tempfile.mkdtemp(), "e2e.command")
        with open(script, "w") as f:
            f.write(f"#!/bin/sh\nprintf '\\033]0;{TITLE}\\007'\nIFS= read -r l\nprintf '%s' \"$l\" > '{out_file}'\n")
        os.chmod(script, 0o755)
        return subprocess.Popen(["open", "-a", "Terminal", script])
    sh = f"IFS= read -r l; printf '%s' \"$l\" > '{out_file}'"
    return subprocess.Popen(["xterm", "-T", TITLE, "-geometry", "70x12+120+120", "-fa", "Monospace", "-fs", "14", "-e", "sh", "-c", sh])


def main():
    print(f"ultron-hands e2e on {sys.platform}: {BIN}", flush=True)
    v = subprocess.run([BIN, "--version"], capture_output=True, text=True)
    check("--version", v.returncode == 0 and v.stdout.startswith("ultron-hands"), v.stdout.strip())
    d = subprocess.run([BIN, "doctor"], capture_output=True, text=True)
    print(d.stdout, flush=True)
    check("doctor", d.returncode == 0, d.stdout.strip().splitlines()[-1] if d.stdout.strip() else d.stderr)
    subprocess.run([BIN, "resume"], capture_output=True)

    s = Server()
    init = s.rpc("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "e2e", "version": "1"}})
    check("initialize", init["result"]["serverInfo"]["name"] == "ultron-hands", init["result"]["serverInfo"])
    s.rpc("notifications/initialized", notify=True)
    tools = [t["name"] for t in s.rpc("tools/list")["result"]["tools"]]
    check("tools/list", len(tools) >= 17, f"{len(tools)} tools")

    err, text, _ = s.call("screen_info")
    check("screen_info", not err, text)
    err, text, imgs = s.call("screenshot")
    check("screenshot", not err and imgs, text)
    save(imgs, "desktop")
    scale = float(re.search(r"scale ([\d.]+)", text).group(1)) if "scale" in text else 1.0

    out_file = os.path.join(tempfile.mkdtemp(), "typed.txt")
    reader_window(out_file)
    win = None
    for _ in range(30):  # slow VMs take a while to open a terminal window
        time.sleep(1)
        err, text, _ = s.call("list_windows")
        try:
            wins = json.loads(text)
            if isinstance(wins, dict):
                wins = wins.get("result", wins.get("windows", []))
            win = next((w for w in wins if TITLE in str(w.get("title", ""))), None)
        except ValueError:
            pass
        if win:
            time.sleep(1)
            break
    check("list_windows finds the test window", win is not None, win or text[:200])

    err, text, imgs = s.call("screenshot")
    save(imgs, "window-open")
    if win:
        x = round((win["x"] + win["w"] / 2) / scale)
        y = round((win["y"] + win["h"] / 2) / scale)
        err, text, imgs = s.call("click", x=x, y=y)
        check("click into the window", not err, text)
        save(imgs, "clicked")
    err, text, _ = s.call("type_text", text=TEXT, look=False)
    check("type_text", not err, text)
    err, text, imgs = s.call("screenshot")
    save(imgs, "typed")
    err, text, _ = s.call("key", keys="enter", look=False)
    check("key enter", not err, text)
    for _ in range(20):
        if os.path.exists(out_file) and os.path.getsize(out_file):
            break
        time.sleep(0.5)
    got = open(out_file).read() if os.path.exists(out_file) else None
    check("the window received exactly the typed text", got == TEXT, repr(got))

    # Teach overlay: a captioned box the person sees but the agent's screenshots don't.
    if sys.platform in ("linux", "win32"):
        err, text, _ = s.call("screenshot")
        err, text, _ = s.call("highlight", x0=200, y0=200, x1=420, y1=260, caption="Click here", seconds=8)
        check("highlight shows a box", not err and "Showing a box" in text, text[:150])
        time.sleep(0.5)
        if sys.platform == "win32":
            import ctypes
            u = ctypes.windll.user32
            found = []
            proc = ctypes.WINFUNCTYPE(ctypes.c_bool, ctypes.c_void_p, ctypes.c_void_p)(
                lambda h, _: found.append(h) or True if u.IsWindowVisible(h) and _cls(u, h) == "UltronHandsOverlay" else True)
            u.EnumWindows(proc, 0)
            check("overlay windows are on screen", len(found) >= 2, f"{len(found)} visible")
        err, text, imgs = s.call("screenshot")
        save(imgs, "agent-view-with-highlight")
        err, text, _ = s.call("highlight", x0=0, y0=0, x1=1, y1=1, seconds=0)
        check("highlight clears", not err, text[:80])

    subprocess.run([BIN, "stop"], capture_output=True)
    err, text, _ = s.call("move_mouse", x=10, y=10)
    check("kill switch blocks input", err and "Kill switch" in text, text[:120])
    subprocess.run([BIN, "resume"], capture_output=True)
    err, text, _ = s.call("move_mouse", x=12, y=12)
    check("resume allows input again", not err, text)

    s.close()
    log = subprocess.run([BIN, "log", "5"], capture_output=True, text=True)
    check("log lists the actions", "type_text" in log.stdout or "move_mouse" in log.stdout, log.stdout.strip()[-200:])
    agents = subprocess.run([BIN, "agents"], capture_output=True, text=True)
    print(agents.stdout, flush=True)
    check("agents", agents.returncode == 0)

    with open(os.path.join(OUT, "results.json"), "w") as f:
        json.dump({"platform": sys.platform, "results": results}, f, indent=2)
    failed = [r["check"] for r in results if not r["ok"]]
    print(f"\n{len(results) - len(failed)}/{len(results)} passed" + (f"; failed: {failed}" if failed else ""), flush=True)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
