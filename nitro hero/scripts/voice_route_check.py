"""Proves which real devices a route reaches: one real engine receives a synthetic tone from
a second engine, with its own output volume at zero so nothing is audible, and Windows is
asked which endpoints that process has running audio on. No account or call is involved.
Pass an input id and an output id to check them; each check uses fresh engines, as a call does."""
import json, os, queue, subprocess, sys, threading, time
from pathlib import Path

exe = Path(os.environ["TEMP"]) / "nh/debug/nitro-voice-engine.exe"


class Engine:
    def __init__(self, events, index, *args):
        self.process = subprocess.Popen([str(exe), *args], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL, text=True, encoding="utf-8", creationflags=subprocess.CREATE_NO_WINDOW)
        threading.Thread(target=lambda: [events.put((index, json.loads(l))) for l in self.process.stdout], daemon=True).start()

    def send(self, value):
        self.process.stdin.write(json.dumps(value) + "\n")
        self.process.stdin.flush()

    def close(self):
        self.process.stdin.close()
        try:
            self.process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.process.kill()


def pump(events, engines, until, seconds=20):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        try:
            index, value = events.get(timeout=.1)
        except queue.Empty:
            continue
        if value["event"] == "error":
            raise AssertionError(value["message"])
        if value["event"] == "description":
            engines[1 - index].send({"command": "description", "description": value["description"]})
        elif value["event"] == "candidate":
            engines[1 - index].send({"command": "candidate", "candidate": value["candidate"]})
        elif value["event"] == "first-decoded" and value.get("side") == "remote":
            engines[index].send({"command": "remote-ready", "epoch": value["epoch"]})
        if until(index, value):
            return value
    raise AssertionError("timed out")


def device_names():
    events = queue.Queue()
    engine = Engine(events, 0)
    engine.send({"command": "devices"})
    devices = pump(events, [engine], lambda i, v: v["event"] == "devices")
    engine.close()
    return devices


def check(label, input_id, output_id, names):
    events = queue.Queue()
    real, tone = Engine(events, 0), Engine(events, 1, "--self-test-source")
    engines = [real, tone]
    try:
        tone.send({"command": "prepare"})
        real.send({"command": "output-level", "level": 0})
        real.send({"command": "prepare", "input": input_id, "output": output_id})
        seen = set()
        pump(events, engines, lambda i, v: (v["event"] == "prepared" and seen.add(i), len(seen) == 2)[1])
        real.send({"command": "peer", "polite": True, "epoch": "route"})
        tone.send({"command": "peer", "polite": False, "epoch": "route"})
        pump(events, engines, lambda i, v: i == 0 and v["event"] == "first-decoded" and v.get("side") == "remote")
        time.sleep(1.5)
        real.send({"command": "route-check"})
        active = pump(events, engines, lambda i, v: v["event"] == "route-check")["active"]
        print(label)
        print("  asked   ", names.get(input_id, "default"), "|", names.get(output_id, "default"))
        print("  running ", [names.get(a, a) for a in active])
        return active
    finally:
        tone.close()
        real.close()


devices = device_names()
names = {d["id"]: d["name"] for d in devices["inputs"] + devices["outputs"]}
failed = False
if len(sys.argv) == 3:
    wanted = sys.argv[1:3]
    if not set(wanted) <= set(check("saved devices", wanted[0], wanted[1], names)):
        failed = True
else:
    mic = devices["inputs"][0]["id"]
    for output in devices["outputs"]:
        if output["id"] not in check("output", mic, output["id"], names):
            failed = True
check("Default", "", "", names)
check("unplugged device falls back to default", "{gone}", "{gone}", names)
print("FAILED" if failed else "all routes reached the right device")
