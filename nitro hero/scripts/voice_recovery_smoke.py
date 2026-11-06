"""Faults that used to end a call, against two real engines with synthetic audio. Nothing
here uses a microphone, a speaker or an account."""
import json, os, queue, subprocess, threading, time
from pathlib import Path

exe = Path(os.environ["TEMP"]) / "nh/debug/nitro-voice-engine.exe"
events = queue.Queue()
peers = []
deliver = [True]
warnings = []
states = []


def send(index, value):
    peers[index].stdin.write(json.dumps(value) + "\n")
    peers[index].stdin.flush()


def read(index, process):
    for line in process.stdout:
        events.put((index, json.loads(line)))


def pump(until, seconds=15, what="step"):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        try:
            index, value = events.get(timeout=.1)
        except queue.Empty:
            continue
        event = value["event"]
        if event == "error":
            raise AssertionError(f"call-ending error during {what}: {value['message']}")
        if event == "warning":
            warnings.append(value)
        if event == "state":
            states.append((index, value["state"]))
        if deliver[0]:
            if event == "description":
                send(1 - index, {"command": "description", "description": value["description"]})
            elif event == "candidate":
                send(1 - index, {"command": "candidate", "candidate": value["candidate"]})
        if until(index, value):
            return value
    raise AssertionError(f"timed out: {what}; warnings: {warnings[-8:]}; states: {states[-8:]}")


def hearing(listener):
    return lambda i, v: i == listener and v["event"] == "level" and v.get("side") == "remote" and v["level"] > .05


def both_hear(what):
    # Only fresh sound counts, not what was already waiting in the queue.
    pump(lambda i, v: False if False else True, what=what + " (settle)")
    deadline = time.monotonic() + .5
    while time.monotonic() < deadline:
        pump(lambda i, v: True, seconds=1, what=what + " (settle)")
    for process in peers:
        assert process.poll() is None, "an engine died during " + what
    pump(hearing(0), what=what + " (0 hears)")
    pump(hearing(1), what=what + " (1 hears)")


def peer(index):
    send(index, {"command": "peer", "polite": index == 1, "epoch": "local-test"})


results = {}
try:
    for index in range(2):
        process = subprocess.Popen([str(exe), "--self-test-source"], stdin=subprocess.PIPE,
            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True,
            creationflags=subprocess.CREATE_NO_WINDOW)
        peers.append(process)
        threading.Thread(target=read, args=(index, process), daemon=True).start()
    ready = set()
    for index in range(2):
        send(index, {"command": "prepare"})
    pump(lambda i, v: (v["event"] == "prepared" and ready.add(i), len(ready) == 2)[1], what="prepare")

    # The first offer is lost, as when it arrived before the member list showed them, and
    # its candidates with it. The impolite side now sits on an offer nobody answers, and
    # ignores the other side's. A restart has to clear it.
    deliver[0] = False
    peer(0)
    pump(lambda i, v: i == 0 and v["event"] == "description", what="lost offer")
    time.sleep(.5)
    while not events.empty():
        events.get()
    deliver[0] = True
    peer(1)
    pump(lambda i, v: i == 1 and v["event"] == "description", what="polite offer ignored")
    send(0, {"command": "restart", "relay": False})
    both_hear("restart clears an unanswered offer")
    results["unansweredOfferRecovers"] = "pass"

    # Stale signaling must never end the call.
    send(0, {"command": "candidate", "candidate": {"candidate": "candidate:1 1 udp 1 192.0.2.1 9 typ host ufrag stale", "sdpMid": "0", "sdpMLineIndex": 0}})
    send(0, {"command": "description", "description": {"type": "answer", "sdp": "v=0\r\n"}})
    send(0, {"command": "description", "description": {"type": "offer", "sdp": "not sdp"}})
    both_hear("stale signaling")
    results["staleSignalingSurvives"] = "pass"

    # Forcing the relay with no relay configured reports nothing fatal, then direct returns.
    send(0, {"command": "restart", "relay": True})
    time.sleep(1)
    send(0, {"command": "restart", "relay": False})
    both_hear("relay forced then direct")
    results["relayToggleSurvives"] = "pass"

    # Stats arrive with real numbers.
    send(1, {"command": "stats"})
    health = pump(lambda i, v: i == 1 and v["event"] == "audio-health", what="stats")
    assert health["received"] > 0, health
    results["audioHealthReceived"] = health["received"]

    # A dead engine command doesn't take the engine with it.
    send(0, {"command": "routes", "input": "", "output": ""})
    warning = pump(lambda i, v: i == 0 and v["event"] == "warning" and v.get("command") == "routes", what="routes warning")
    both_hear("after a failed route change")
    results["failedRouteSurvives"] = "pass"
    print(json.dumps(results))
finally:
    for process in peers:
        process.stdin.close()
    for process in peers:
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
