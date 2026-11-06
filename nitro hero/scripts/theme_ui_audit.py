"""Read-only MCP UI checks. Captures stay local; may include private chat media."""
import json
import os
import time
import sys
import urllib.request
from pathlib import Path
from PIL import Image, ImageDraw

hero = Path(__file__).resolve().parents[1]
descriptor = json.loads((Path(os.environ['LOCALAPPDATA']) / 'Nitro Hero/dev-mcp.json').read_text())
output = hero / 'audits/theme-ui'
output.mkdir(parents=True, exist_ok=True)

def call(tool_name, **arguments):
    body = {'jsonrpc': '2.0', 'id': 1, 'method': 'tools/call', 'params': {'name': tool_name, 'arguments': arguments}}
    request = urllib.request.Request(descriptor['url'], json.dumps(body).encode(),
        {'Authorization': 'Bearer ' + descriptor['token'], 'Content-Type': 'application/json'})
    reply = json.load(urllib.request.urlopen(request, timeout=15))
    result = reply.get('result', reply)
    if result.get('isError') or reply.get('error'):
        raise RuntimeError('MCP call failed: ' + tool_name)
    return result.get('structuredContent', result)

initial = call('nitro_get_state')
assert not initial['call_connected'], 'Do not navigate a live call for a theme audit.'
try:
    for theme in (sys.argv[1:] or ['default', 'amethyst', 'gold']):
        call('nitro_set_theme', name=theme)
        call('nitro_navigate', view='gallery')
        call('nitro_wheel', x=700, y=500, notches=50)
        call('nitro_wheel', x=700, y=500, notches=-5)
        time.sleep(.25)
        gallery = Image.open(call('nitro_screenshot')['path']).copy()
        gallery.save(output / f'{theme}-gallery.png')
        call('nitro_navigate', view='chat')
        time.sleep(.25)
        chat = Image.open(call('nitro_screenshot')['path']).copy()
        chat.save(output / f'{theme}-chat.png')
        # Keep controls at original resolution: gallery + sidebar + composer.
        sheet = Image.new('RGB', (1240, 860), '#101010')
        sheet.paste(gallery.crop((190, 0, 1080, 760)), (0, 30))
        sheet.paste(chat.crop((chat.width - 330, 30, chat.width, 680)), (900, 30))
        sheet.paste(chat.crop((150, 720, 790, 815)), (0, 760))
        ImageDraw.Draw(sheet).text((12, 10), theme + ' — gallery, sidebar, composer', fill='white')
        sheet.save(output / f'{theme}-review.png')
        print(theme + ': gallery and chat captured')
finally:
    call('nitro_set_theme', name=initial['theme'])
    call('nitro_navigate', view=initial['view'])
