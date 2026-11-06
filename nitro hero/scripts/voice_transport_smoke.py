"""Two real native WebRTC peers with synthetic PCM. No microphone/speaker/network account."""
import json, os, queue, subprocess, threading, time
from pathlib import Path

exe = Path(os.environ["TEMP"]) / "nh/debug/nitro-voice-engine.exe"
events = queue.Queue()
peers = []
def send(index, value):
    peers[index].stdin.write(json.dumps(value) + "\n")
    peers[index].stdin.flush()
def read(index, process):
    for line in process.stdout:
        events.put((index, json.loads(line)))
def pump(until, seconds=15):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        try:
            index, value = events.get(timeout=.1)
        except queue.Empty:
            continue
        event = value["event"]
        if event == "error":
            raise AssertionError(value["message"])
        if event == "description":
            send(1-index, {"command":"description", "description":value["description"]})
        elif event == "candidate":
            send(1-index, {"command":"candidate", "candidate":value["candidate"]})
        if until(index, value):
            return
    raise AssertionError("Native voice acceptance timed out")
try:
    for index in range(2):
        process = subprocess.Popen([str(exe), "--self-test-source"], stdin=subprocess.PIPE,
            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True,
            creationflags=subprocess.CREATE_NO_WINDOW)
        peers.append(process)
        threading.Thread(target=read, args=(index,process),daemon=True).start()
    prepared=set()
    for index in range(2): send(index,{"command":"prepare"})
    def readiness(index,value):
        if value["event"]=="prepared": prepared.add(index)
        return len(prepared)==2
    pump(readiness)
    for index in range(2): send(index,{"command":"peer","polite":index==1,"epoch":"local-test"})
    connected=set(); decoded=set(); signal_start=time.monotonic()
    def audio(index,value):
        if value["event"]=="state" and value["state"]=="connected": connected.add(index)
        if value["event"]=="level" and value.get("side")=="remote" and value["level"]>.05: decoded.add(index)
        return len(connected)==2 and len(decoded)==2
    pump(audio)
    connect_ms=round((time.monotonic()-signal_start)*1000)
    send(0,{"command":"controls","muted":True,"deafened":False})
    pump(lambda i,v:i==1 and v["event"]=="level" and v.get("side")=="remote" and v["level"]<.01)
    send(0,{"command":"controls","muted":False,"deafened":False})
    pump(lambda i,v:i==1 and v["event"]=="level" and v.get("side")=="remote" and v["level"]>.05)
    send(0,{"command":"restart"})
    # Continue delivering SDP/ICE until a post-restart description plus decoded PCM.
    restarted=[False]
    def recovered(i,v):
        if i==0 and v["event"]=="description":restarted[0]=True
        return restarted[0] and i==1 and v["event"]=="level" and v.get("side")=="remote" and v["level"]>.05
    pump(recovered)
    print(json.dumps({"twoNativePeers":"pass","simultaneousOffers":"pass","twoWayDecodedPcm":"pass","muteUnmute":"pass","restartSignaling":"pass","connectAndDecodeMs":connect_ms,"audibleOutput":"not measured"}))
finally:
    for process in peers:
        process.stdin.close()
    for process in peers:
        try:process.wait(timeout=10)
        except subprocess.TimeoutExpired:process.kill();process.wait();raise AssertionError("Voice engine failed EOF cleanup")
