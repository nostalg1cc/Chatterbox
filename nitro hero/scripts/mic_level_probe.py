"""Explicitly authorized local volume-only check. Never saves audio or joins voice."""
import argparse
import json
import os
from pathlib import Path
import queue
import subprocess
import threading
import time

args = argparse.ArgumentParser()
args.add_argument('--allow-microphone', action='store_true', required=True)
args.add_argument('--seconds', type=float, default=5)
args = args.parse_args()
assert 1 <= args.seconds <= 10
root = Path(__file__).resolve().parents[1]
preferences = json.loads((Path(os.environ['LOCALAPPDATA']) / 'Nitro Hero/preferences.json').read_text())['preferences']
route = preferences.get('input_device_id', 'default')
process = subprocess.Popen([str(root / 'target/debug/nitro-voice-engine.exe')],
    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
    text=True, creationflags=subprocess.CREATE_NO_WINDOW)
events = queue.Queue()
def read():
    for line in process.stdout:
        try: events.put(json.loads(line))
        except ValueError: pass
threading.Thread(target=read, daemon=True).start()
process.stdin.write(json.dumps({'command': 'prepare', 'input': '' if route == 'default' else route, 'output': ''}) + '\n')
process.stdin.flush()
levels, peaks, errors, statuses = [], [], [], []
prepared = False
try:
    end = time.monotonic() + args.seconds
    while time.monotonic() < end:
        try: event = events.get(timeout=.1)
        except queue.Empty: continue
        if event.get('event') == 'prepared': prepared = True
        if event.get('event') == 'error': errors.append(event.get('message'))
        if event.get('event') == 'local-meter': statuses.append(event.get('status'))
        if event.get('event') == 'level' and event.get('side') == 'local':
            levels.append(event['level'])
            peaks.append(event.get('raw_peak', 0))
finally:
    process.stdin.close()
    try: process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait()
        errors.append('worker did not stop on EOF')
print(json.dumps({'prepared': prepared, 'meter': statuses, 'levelSamples': len(levels),
    'nonzeroSamples': sum(level > .002 for level in levels), 'peak': max(levels, default=0),
    'rawPeak': max(peaks, default=0),
    'errors': errors, 'audioSaved': False, 'callOpened': False}))
