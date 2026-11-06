"""Exercise the IMAGE path, comparing red portrait and green rim pixels.

No microphone, calls or backend writes. Run with a debug Hero --dev-mcp instance.
Refuses during a real call. Captures are calibration graphics, not chat content.
"""
import json
from pathlib import Path
import subprocess
import sys
from PIL import Image

hero = Path(__file__).resolve().parents[1]
output = hero / 'audits/overlay-alignment'
output.mkdir(parents=True, exist_ok=True)

def call(name, **arguments):
    reply = subprocess.run([sys.executable, str(hero / 'scripts/mcp_call.py'), name,
        json.dumps(arguments)], check=True, capture_output=True, text=True, encoding='utf-8')
    result = json.loads(reply.stdout)
    if result.get('isError'):
        raise RuntimeError('MCP refused ' + name)
    return result

def centroid(image, photo):
    weight = sx = sy = 0
    for y in range(image.height):
        for x in range(image.width):
            r, g, b = image.getpixel((x, y))
            value = max(r - max(g, b), 0) if photo else max(g - max(r, b), 0)
            weight += value
            sx += value * x
            sy += value * y
    return [sx / weight, sy / weight] if weight else None

assert not call('nitro_get_state')['call_connected'], 'Leave the real call first.'
results = []
for scale in [.5, .7, 1.5]:
    frames = call('nitro_test_overlay', scale=scale, marker=True,
        levels=[0, .05, .1, .2, .4, .6, .8, 1, .1, 0])['frames']
    baseline = None
    for index, frame in enumerate(frames):
        image = Image.open(frame['path']).convert('RGB')
        # Copy NOW: the next probe reuses temporary frame filenames.
        image.save(output / f'{scale}-{index}.png')
        photo, ring = centroid(image, True), centroid(image, False)
        assert photo is not None, 'Photo marker failed to render; fallback is NOT a valid test.'
        baseline = baseline or photo
        drift = max(abs(a - b) for a, b in zip(photo, baseline))
        separation = max(abs(a - b) for a, b in zip(photo, ring)) if ring else None
        row = dict(scale=scale, level=frame['level'], photo=photo, ring=ring,
            drift=drift, separation=separation)
        results.append(row)
        assert drift < .1, row
        if frame['level'] >= .05:
            assert separation is not None and separation < .1, row
(output / 'measurements.json').write_text(json.dumps(results, indent=2))
print(json.dumps({'frames': len(results), 'maxPhotoDriftPixels': max(r['drift'] for r in results),
    'maxRingSeparationPixels': max(r['separation'] or 0 for r in results), 'microphone': False}))
