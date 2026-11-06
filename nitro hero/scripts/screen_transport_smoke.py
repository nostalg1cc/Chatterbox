"""Two native engines share a synthetic moving picture over a local WebRTC connection.

Never captures the real screen and uses no account. Checks the frames that reach the
private display socket, and that stopping the share ends them.
"""
import json, os, queue, secrets, socket, struct, subprocess, sys, threading, time
from pathlib import Path

# Pass "release" to measure real frame rates. Debug builds draw the test picture slowly.
exe = Path(os.environ["TEMP"]) / f"nh/{sys.argv[1] if len(sys.argv) > 1 else 'debug'}/nitro-voice-engine.exe"
events = queue.Queue()
peers = []
frames = []
preview = []
done = threading.Event()


def send(index, value):
    peers[index].stdin.write(json.dumps(value) + "\n")
    peers[index].stdin.flush()


def read(index, process):
    for line in process.stdout:
        events.put((index, json.loads(line)))


def wait(until, seconds=20):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        try:
            index, value = events.get(timeout=0.1)
        except queue.Empty:
            continue
        if value["event"] == "error":
            raise AssertionError(value["message"])
        result = until(index, value)
        if result:
            return result
    raise AssertionError("Screen transport timed out")


def exact(connection, count):
    data = b""
    while len(data) < count:
        chunk = connection.recv(count - len(data))
        if not chunk:
            raise ConnectionError
        data += chunk
    return data


def receive(listener, token, into):
    connection, _ = listener.accept()
    if exact(connection, len(token)) != token.encode():
        raise AssertionError("Wrong display token")
    try:
        while not done.is_set():
            width, height = struct.unpack("<II", exact(connection, 8))
            pixels = exact(connection, width * height * 4)
            into.append((time.monotonic(), width, height, pixels[0], pixels[3]))
    except ConnectionError:
        pass


def listen(into):
    listener = socket.socket()
    listener.bind(("127.0.0.1", 0))
    listener.listen(1)
    token = secrets.token_hex(16)
    threading.Thread(target=receive, args=(listener, token, into), daemon=True).start()
    return listener.getsockname()[1], token


try:
    for index in range(2):
        process = subprocess.Popen(
            [str(exe), "--self-test-source"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL, text=True, creationflags=subprocess.CREATE_NO_WINDOW)
        peers.append(process)
        threading.Thread(target=read, args=(index, process), daemon=True).start()
    port, token = listen(frames)
    preview_port, preview_token = listen(preview)
    started = time.monotonic()
    send(0, {"command": "screen-publish", "kind": "screen", "id": "0",
             "previewPort": preview_port, "previewToken": preview_token})
    offer = wait(lambda i, v: v if v["event"] == "screen-offer" else None)
    assert "a=candidate:" in offer["sdp"], "The offer carries no gathered candidates"
    send(1, {"command": "screen-view", "sdp": offer["sdp"],
             "port": port, "token": token})
    answer = wait(lambda i, v: v if v["event"] == "screen-view-answer" else None)
    send(0, {"command": "screen-answer", "sdp": answer["sdp"]})
    wait(lambda i, v: v["event"] == "screen-first-frame")
    first_ms = round((time.monotonic() - started) * 1000)
    time.sleep(6)
    received = list(frames)
    assert len(received) > 20, f"Only {len(received)} frames arrived"
    span = received[-1][0] - received[0][0]
    rate = (len(received) - 1) / span
    assert rate <= 31, f"Display rate is uncapped: {rate:.1f}/s"
    assert all(frame[4] == 255 for frame in received), "Pixels aren't opaque BGRA"
    assert len({frame[3] for frame in received}) > 3, "The picture isn't moving"
    # WebRTC starts small while it learns the connection, then climbs to full size.
    sizes = sorted({frame[1:3] for frame in received})
    assert received[-1][1:3] == (1280, 720), f"Never reached full size: {sizes}"
    full_at = next(frame[0] for frame in received if frame[1:3] == (1280, 720)) - received[0][0]
    assert len(preview) > 10, f"Only {len(preview)} preview frames arrived"
    preview_rate = (len(preview) - 1) / (preview[-1][0] - preview[0][0])
    assert preview_rate <= 16, f"Preview rate is uncapped: {preview_rate:.1f}/s"
    send(0, {"command": "screen-stop"})
    time.sleep(1.5)
    count = len(frames)
    time.sleep(1)
    assert len(frames) == count, "Frames kept arriving after the share stopped"
    print(json.dumps({"firstFrameMs": first_ms, "framesPerSecond": round(rate, 1),
                      "previewPerSecond": round(preview_rate, 1),
                      "size": list(received[-1][1:3]), "sizes": sizes, "fullSizeAfterSeconds": round(full_at, 2),
                      "stopEndsFrames": "pass",
                      "realScreen": "not used"}))
finally:
    done.set()
    for process in peers:
        process.kill()
