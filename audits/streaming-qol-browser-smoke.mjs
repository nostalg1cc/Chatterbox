// Isolated production-component checks. Synthetic media and sound records;
// no account, actual display capture, audible output, or backend writes.
import { createRequire, stripTypeScriptTypes } from 'node:module';
import { readFile } from 'node:fs/promises';
import { createServer } from 'vite';
import assert from 'node:assert/strict';

const { chromium } = createRequire(import.meta.url)(process.argv[2] || 'playwright');
const fixture = `<!doctype html><div id="fixture" style="position:absolute;top:40px;left:500px;display:flex;align-items:center;gap:10px"></div><script type="module">
import React from '/node_modules/.vite/deps/react.js';
import ReactDOM from '/node_modules/.vite/deps/react-dom_client.js';
import { SoundboardDropdown } from '/src/features/v3-shell/components/SoundboardDropdown.jsx';
import { WebHeaderControls } from '/src/features/v3-shell/components/WebHeaderControls.jsx';
import { useAuth } from '/src/stores/auth.ts';
import { useSoundboard } from '/src/stores/soundboard.ts';
import { useProfiles } from '/src/stores/profiles.ts';
import { usePreferences } from '/src/stores/preferences.ts';
import '/src/features/v3-shell/styles.css';
useAuth.setState({ status: 'signedIn', userId: 'synthetic', profile: null });
useProfiles.setState({ ensure: async () => {} });
usePreferences.setState({ pinnedSoundIds: ['a'], soundboardVolume: 50 });
useSoundboard.setState({ availableSounds: [
 { id: 'a', name: 'Beat to My Melody', owner_id: 'synthetic', created_at: '2026-01-01', duration_ms: 5000 },
 { id: 'b', name: 'Another World', owner_id: 'synthetic', created_at: '2026-01-02', duration_ms: 3000 },
 { id: 'c', name: 'World of Dreams', owner_id: 'synthetic', created_at: '2026-01-03', duration_ms: 4000 }
], loadAvailable: async () => {}, play: async id => { window.fixturePlayed = id; } });
ReactDOM.createRoot(document.getElementById('fixture')).render(React.createElement(React.Fragment, null, React.createElement(SoundboardDropdown, { conversationId: 'synthetic', isMenuOpen: true, onMenuOpenChange() {} }), React.createElement(WebHeaderControls)));
</script>`;
const server = await createServer({
  server: { port: 0, host: '127.0.0.1', strictPort: false },
  logLevel: 'error',
  plugins: [{ name: 'isolated-qol-fixture', configureServer(server) {
    server.middlewares.use('/__nitro_synthetic__', async (_, response) => { response.setHeader('Content-Type', 'text/html'); response.end(await server.transformIndexHtml('/__nitro_synthetic__', fixture)); });
  } }],
});
let browser;
try {
  await server.listen();
  const origin = server.resolvedUrls.local[0];
  browser = await chromium.launch({ channel: 'chrome', headless: true, args: ['--autoplay-policy=no-user-gesture-required'] });
  const page = await browser.newPage({ viewport: { width: 1000, height: 720 } });
  const errors = []; page.on('pageerror', error => { errors.push(error.message); console.error(error.message); });
  await page.route('**/*', route => route.request().url().startsWith(origin) ? route.continue() : route.abort());
  await page.goto(origin + '__nitro_synthetic__');
  console.log('Synthetic component fixture loaded');
  const search = page.getByRole('searchbox', { name: 'Search sounds' });
  await search.waitFor();
  assert.equal(await page.locator('.soundboard-widget').count(), 3);
  await search.fill('  WORLD ');
  await page.waitForFunction(() => document.querySelectorAll('.soundboard-widget').length === 2);
  await page.getByRole('menuitem', { name: 'Play Another World' }).click();
  assert.equal(await page.evaluate(() => window.fixturePlayed), 'b');
  await search.fill('no-such-sound');
  await page.getByRole('status').filter({ hasText: 'No matching sounds' }).waitFor();
  await page.getByRole('button', { name: 'Clear sound search' }).click();
  await page.waitForFunction(() => document.querySelectorAll('.soundboard-widget').length === 3);
  console.log('PASS: actual soundboard component filters case-insensitively, trims input, preserves playback, clears and shows no-match feedback');
  await page.getByRole('button', { name: 'Hide media sidebar', exact: true }).click();
  await page.getByRole('button', { name: 'Show media sidebar', exact: true }).waitFor();
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('button', { name: 'Close settings', exact: true }).click();
  await page.getByRole('button', { name: 'Settings', exact: true }).waitFor();
  console.log('PASS: web-only header settings and media-sidebar actions use the existing app state');

  const relay = (await readFile('src/lib/voice-hud-stream.ts', 'utf8')).replace(/^import .*;\r?\n/gm, '').replace(/export /g, '');
  const layout = (await readFile('src/lib/screen-share-layout.ts', 'utf8')).replace(/export /g, '');
  const pixels = await page.evaluate(async code => {
    const source = document.createElement('canvas'); source.width = 2560; source.height = 1080;
    const context = source.getContext('2d'); context.fillStyle = '#40c080'; context.fillRect(0, 0, 2560, 1080);
    const stream = source.captureStream(10);
    let finish; const received = new Promise(resolve => { finish = resolve; });
    const factory = new Function('emitTo', 'isTauri', code + ';return relayVoiceHudStream;');
    const stop = factory(async (_, __, frame) => finish(frame.image), true)('synthetic', stream);
    try {
      const image = new Image();
      image.src = await Promise.race([received, new Promise((_, reject) => setTimeout(() => reject(Error('No overlay frame')), 5000))]);
      await image.decode();
      const canvas = document.createElement('canvas'); canvas.width = 320; canvas.height = 180;
      const output = canvas.getContext('2d'); output.drawImage(image, 0, 0);
      return [output.getImageData(160, 0, 1, 1).data[1], output.getImageData(160, 179, 1, 1).data[1]];
    } finally { stop(); stream.getTracks().forEach(track => track.stop()); }
  }, stripTypeScriptTypes(layout + '\n' + relay));
  assert(pixels.every(green => green > 150), 'ultrawide overlay must fill both top and bottom without black bars');
  assert.deepEqual(errors, []);
  console.log('PASS: actual overlay relay fills a 16:9 frame from an ultrawide capture without black bars');
  const native = await browser.newPage();
  await native.addInitScript(() => { window.__TAURI_INTERNALS__ = {}; });
  await native.route('**/*', route => route.request().url().startsWith(origin) ? route.continue() : route.abort());
  await native.goto(origin + '__nitro_synthetic__');
  await native.getByRole('searchbox', { name: 'Search sounds' }).waitFor();
  assert.equal(await native.locator('.web-header-control').count(), 0);
  console.log('PASS: extra header controls are absent in native mode');
} finally { await browser?.close(); await server.close(); }
