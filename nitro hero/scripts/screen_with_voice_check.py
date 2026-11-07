"""A real call has voice running when a screen is shared and watched. Two engines with
synthetic audio and a synthetic picture, no account, microphone or real screen."""
import json, os, queue, secrets, socket, struct, subprocess, sys, threading, time
from pathlib import Path

exe = Path(os.environ["TEMP"]) / f"nh/{sys.argv[1] if len(sys.argv) > 1 else 'debug'}/nitro-voice-engine.exe"
events = queue.Queue()
peers = []


def send(index, value):
    peers[index].stdin.write(json.dumps(value) + "\n")
    peers[index].stdin.flush()


def read(index, process):
    for line in process.stdout:
        events.put((index, json.loads(line)))


def exact(connection, count):
    data = b""
    while len(data) < count:
        chunk = connection.recv(count - len(data))
        if not chunk:
            raise ConnectionError
        data += chunk
    return data


def listen(frames):
    listener = socket.socket()
    listener.bind(("127.0.0.1", 0))
    listener.listen(1)
    token = secrets.token_hex(16)

    def receive():
        connection, _ = listener.accept()
        assert exact(connection, len(token)) == token.encode()
        try:
            while True:
                width, height = struct.unpack("<II", exact(connection, 8))
                exact(connection, width * height * 4)
                frames.append((time.monotonic(), width, height))
        except (ConnectionError, OSError):
            pass

    threading.Thread(target=receive, daemon=True).start()
    return listener.getsockname()[1], token


def run(seconds, label, publisher=None):
    """Passes signaling between the two sides and reports what each one heard."""
    heard = [0.0, 0.0]
    notes = []
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        try:
            index, value = events.get(timeout=.05)
        except queue.Empty:
            continue
        event = value["event"]
        if event == "description":
            send(1 - index, {"command": "description", "description": value["description"]})
        elif event == "candidate":
            send(1 - index, {"command": "candidate", "candidate": value["candidate"]})
        elif event == "first-decoded" and value.get("side") == "remote":
            send(index, {"command": "remote-ready", "epoch": value["epoch"]})
        elif event == "level" and value.get("side") == "remote" and end - time.monotonic() < 3:
            heard[index] = max(heard[index], value["level"])
        elif event == "screen-offer":
            send(1 - index, {"command": "screen-view", "sdp": value["sdp"], "port": watch_port, "token": watch_token})
        elif event == "screen-view-answer":
            send(1 - index, {"command": "screen-answer", "sdp": value["sdp"]})
        elif event.startswith("screen-") or event in ("warning", "error"):
            notes.append(f"{index} {event} {value.get('state', '')}{value.get('message', '')}".strip())
    print(f"{label}: heard {[round(level, 3) for level in heard]}\n   {notes}")
    return heard


try:
    for index in range(2):
        process = subprocess.Popen([str(exe), "--self-test-source"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL, text=True, encoding="utf-8", creationflags=subprocess.CREATE_NO_WINDOW)
        peers.append(process)
        threading.Thread(target=read, args=(index, process), daemon=True).start()
    for index in range(2):
        send(index, {"command": "prepare"})
    run(.5, "prepare")
    for index in range(2):
        send(index, {"command": "peer", "polite": index == 1, "epoch": "e"})
    before = run(5, "voice alone")
    assert min(before) > .05, "voice never connected"

    results = {}
    for sharer, name in ((0, "impolite side shares"), (1, "polite side shares")):
        frames = []
        watch_port, watch_token = listen(frames)
        send(sharer, {"command": "screen-publish", "kind": "screen", "id": "0"})
        during = run(8, name)
        recent = [frame for frame in frames if time.monotonic() - frame[0] < 3]
        results[name] = {
            "frames": len(frames),
            "fpsLast3s": round(len(recent) / 3, 1),
            "size": frames[-1][1:] if frames else None,
            "voiceStillHeard": min(during) > .05,
        }
        send(sharer, {"command": "screen-stop"})
        send(1 - sharer, {"command": "screen-view-stop"})
        run(1, "stopped")
    print(json.dumps(results))
    ok = all(result["frames"] > 0 and result["voiceStillHeard"] for result in results.values())
    print("PASS" if ok else "FAIL")
    sys.exit(0 if ok else 1)
finally:
    for process in peers:
        process.stdin.close()
    for process in peers:
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
