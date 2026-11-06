"""Can each side still hear the other after one side reconnects? Uses synthetic audio, except
`mic`, which sends the real microphone to a test engine in this process tree only, with
output volume at zero. No account or call is involved."""
import json, os, queue, subprocess, sys, threading, time
from pathlib import Path

exe = Path(os.environ["TEMP"]) / "nh/debug/nitro-voice-engine.exe"


class Rig:
    def __init__(self):
        self.events = queue.Queue()
        self.engines = {}

    def start(self, name, *args):
        process = subprocess.Popen([str(exe), *args], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL, text=True, encoding="utf-8", creationflags=subprocess.CREATE_NO_WINDOW)
        self.engines[name] = process
        threading.Thread(target=lambda: [self.events.put((name, process, json.loads(l))) for l in process.stdout], daemon=True).start()

    def send(self, name, value):
        self.engines[name].stdin.write(json.dumps(value) + "\n")
        self.engines[name].stdin.flush()

    def stop(self, name):
        process = self.engines.pop(name)
        process.stdin.close()
        process.wait(timeout=10)

    def run(self, seconds, label, gate=True):
        levels = {name: [] for name in self.engines}
        notes = []
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            try:
                name, process, value = self.events.get(timeout=.05)
            except queue.Empty:
                continue
            if self.engines.get(name) is not process:
                continue
            event = value["event"]
            other = next((n for n in self.engines if n != name), None)
            if event == "description" and other:
                self.send(other, {"command": "description", "description": value["description"]})
            elif event == "candidate" and other:
                self.send(other, {"command": "candidate", "candidate": value["candidate"]})
            elif event == "first-decoded" and value.get("side") == "remote" and gate:
                self.send(name, {"command": "remote-ready", "epoch": value["epoch"]})
                notes.append(f"{name} decoded")
            elif event == "level" and value.get("side") == "remote":
                # Only the last 3 seconds count: older readings may be from before the change.
                if end - time.monotonic() < 3:
                    levels[name].append(value["level"])
            elif event in ("warning", "error"):
                notes.append(f"{name} {event}: {value.get('command', '')} {value['message']}")
            elif event == "state":
                notes.append(f"{name} {value['state']}")
            elif event == "audio-health":
                notes.append(f"{name} packets {value['received']}")
        peak = {name: round(max(found), 3) if found else None for name, found in levels.items()}
        print(f"{label}: heard {peak}\n   {notes}")
        return peak


def prepared(rig, *names):
    for name in names:
        rig.send(name, {"command": "prepare"})
    rig.run(.5, "prepare")


def rejoin(impolite_returns):
    """One side gets a new engine while the other keeps its connection."""
    rig = Rig()
    rig.start("a", "--self-test-source")
    rig.start("b", "--self-test-source")
    prepared(rig, "a", "b")
    rig.send("a", {"command": "peer", "polite": False, "epoch": "e"})
    rig.send("b", {"command": "peer", "polite": True, "epoch": "e"})
    rig.run(3, "first connection")
    back, stays = ("a", "b") if impolite_returns else ("b", "a")
    rig.stop(back)
    time.sleep(.3)
    rig.start(back, "--self-test-source")
    prepared(rig, back)
    rig.send(back, {"command": "peer", "polite": not impolite_returns, "epoch": "e"})
    peak = rig.run(6, f"{'impolite' if impolite_returns else 'polite'} side came back on a new engine")
    for name in list(rig.engines):
        rig.stop(name)
    return peak


def mic():
    rig = Rig()
    rig.start("real")
    rig.start("tone", "--self-test-source")
    rig.send("real", {"command": "output-level", "level": 0})
    rig.send("tone", {"command": "prepare"})
    rig.send("real", {"command": "prepare", "input": sys.argv[2] if len(sys.argv) > 2 else "", "output": ""})
    rig.run(1, "prepare")
    rig.send("real", {"command": "controls", "muted": False, "deafened": False})
    rig.send("real", {"command": "peer", "polite": True, "epoch": "e"})
    rig.send("tone", {"command": "peer", "polite": False, "epoch": "e"})
    rig.run(3, "connect")
    rig.send("tone", {"command": "stats"})
    peak = rig.run(6, "real mic as heard by the other side")
    for name in list(rig.engines):
        rig.stop(name)
    return peak


if len(sys.argv) > 1 and sys.argv[1] == "mic":
    mic()
else:
    rejoin(True)
    rejoin(False)
