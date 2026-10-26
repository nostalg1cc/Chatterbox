// Synthetic browser media + built overlay. No accounts, microphone, real
// display capture, audible output, or provider requests. Requires npm run build.
import { createRequire, stripTypeScriptTypes } from 'node:module';
import { readFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { resolve, extname, sep } from 'node:path';
import assert from 'node:assert/strict';

const require = createRequire(import.meta.url);
const { chromium } = require(process.argv[2] || 'playwright');
const root = resolve('dist');
const host = createServer(async (request, response) => {
  const path = resolve(root, '.' + new URL(request.url, 'http://localhost').pathname);
  if (!path.startsWith(root + sep)) { response.writeHead(404).end(); return; }
  try {
    const body = await readFile(path);
    response.setHeader('Content-Type', ({ '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css' })[extname(path)] || 'application/octet-stream');
    response.end(body);
  } catch { response.end('<!doctype html><title>Synthetic stream test</title>'); }
});
await new Promise(resolve => host.listen(0, '127.0.0.1', resolve));
let browser;
try {
  browser = await chromium.launch({ channel: 'chrome', headless: true, args: ['--autoplay-policy=no-user-gesture-required'] });
  const page = await browser.newPage({ viewport: { width: 600, height: 600 } });
  await page.route('**/*', route => route.request().url().startsWith(`http://127.0.0.1:${host.address().port}/`) ? route.continue() : route.abort());
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto(`http://127.0.0.1:${host.address().port}/fixture`);
  const source = (await readFile('src/lib/cloudflare-realtime.ts', 'utf8')).replace(/^import .*;\r?\n/gm, '').replace(/export /g, '');
  const result = await page.evaluate(async code => {
    const sessions = new Map(), peers = [], actions = [];
    const gather = pc => pc.iceGatheringState === 'complete' ? Promise.resolve() : new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(Error('Synthetic ICE gathering timed out')), 4000);
      pc.addEventListener('icegatheringstatechange', () => { if (pc.iceGatheringState === 'complete') { clearTimeout(timer); resolve(); } });
    });
    const supabase = { functions: { invoke: async (_, { body }) => {
      actions.push(body.action);
      if (body.action === 'create') {
        if (body.sessionDescription !== undefined) throw Error('Empty SDP create regression');
        const sessionId = String(sessions.size);
        const peer = new RTCPeerConnection({ iceServers: [], bundlePolicy: 'max-bundle' });
        peer.received = [];
        peer.ontrack = event => peer.received.push(event.track);
        peers.push(peer); sessions.set(sessionId, peer);
        return { data: { sessionId } };
      }
      const peer = sessions.get(body.sessionId);
      if (body.action === 'publish') {
        if (!body.sessionDescription.sdp.includes('a=ice-ufrag:')) throw Error('Missing ICE credentials');
        await peer.setRemoteDescription(body.sessionDescription);
        await peer.setLocalDescription(await peer.createAnswer()); await gather(peer);
        return { data: { sessionDescription: peer.localDescription.toJSON() } };
      }
      if (body.action === 'subscribe') {
        for (const track of sessions.get(body.remoteSessionId).received) peer.addTransceiver(track, { direction: 'sendonly' });
        await peer.setLocalDescription(await peer.createOffer()); await gather(peer);
        return { data: { requiresImmediateRenegotiation: true, sessionDescription: peer.localDescription.toJSON() } };
      }
      await peer.setRemoteDescription(body.sessionDescription);
      return { data: {} };
    } } };
    const factories = new Function('supabase', 'SCREEN_SHARE_MAX_BITRATE', 'recordVoiceEvent', code + '; return {createCloudflareScreenPublisher, createCloudflareScreenSubscriber};')(supabase, 1_000_000, () => {});
    const canvas = document.createElement('canvas'); canvas.width = 640; canvas.height = 360;
    const context = canvas.getContext('2d'); let tick = 0;
    const paint = setInterval(() => { context.fillStyle = tick++ % 2 ? '#d05050' : '#50b080'; context.fillRect(0, 0, 640, 360); }, 50);
    const capture = canvas.captureStream(15);
    const audio = new AudioContext(), oscillator = audio.createOscillator(), destination = audio.createMediaStreamDestination();
    oscillator.connect(destination); oscillator.start(); await audio.resume();
    capture.addTrack(destination.stream.getAudioTracks()[0]);
    let publisher, subscriber;
    try {
      publisher = await factories.createCloudflareScreenPublisher('synthetic', capture, 'voice', []);
      let received;
      subscriber = await factories.createCloudflareScreenSubscriber('synthetic', publisher.sessionId, publisher.trackNames, stream => { received = stream; }, 'voice', []);
      const video = document.createElement('video'); video.muted = true; video.srcObject = received; document.body.append(video); await video.play();
      const deadline = Date.now() + 8000;
      let decoded = 0, audioPackets = 0;
      while (Date.now() < deadline) {
        const stats = await subscriber.getStats();
        stats.forEach(report => {
          if (report.type === 'inbound-rtp' && report.kind === 'video') decoded = report.framesDecoded || 0;
          if (report.type === 'inbound-rtp' && report.kind === 'audio') audioPackets = report.packetsReceived || 0;
        });
        if (decoded > 2 && audioPackets > 0) break;
        await new Promise(resolve => setTimeout(resolve, 100));
      }
      const result = { width: video.videoWidth, height: video.videoHeight, tracks: received.getTracks().map(t => t.kind).sort(), decoded, audioPackets, actions };
      video.pause(); video.srcObject = null;
      return result;
    } finally {
      clearInterval(paint); capture.getTracks().forEach(track => track.stop()); oscillator.stop(); await audio.close();
      publisher?.connection.close(); subscriber?.close(); peers.forEach(pc => pc.close());
    }
  }, stripTypeScriptTypes(source));
  assert.deepEqual(result.actions, ['create', 'publish', 'create', 'subscribe', 'renegotiate']);
  assert.deepEqual(result.tracks, ['audio', 'video']); assert(result.decoded > 2); assert(result.audioPackets > 0); assert(result.width >= 160); assert.equal(result.width / result.height, 16 / 9);
  console.log('PASS: real Chromium WebRTC sends/decodes synthetic screen video and audio through both production factories', JSON.stringify(result));

  await page.addInitScript(() => {
    let id = 0; const callbacks = new Map(), listeners = new Map();
    window.fixtureListeners = listeners;
    window.__TAURI_INTERNALS__ = {
      transformCallback(fn) { callbacks.set(++id, fn); return id; },
      invoke: async (command, args) => {
        if (command === 'plugin:event|listen') { listeners.set(args.handler, args); return args.handler; }
        if (command === 'plugin:event|unlisten') listeners.delete(args.eventId);
      },
    };
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
    window.fixtureEmit = (event, payload) => { for (const [id, listener] of listeners) if (listener.event === event) callbacks.get(id)({ event, id, payload: structuredClone(payload) }); };
  });
  await page.goto(`http://127.0.0.1:${host.address().port}/voice-hud.html`);
  await page.waitForFunction(() => [...window.fixtureListeners.values()].some(listener => listener.event === 'voice-hud:update'));
  const participants = [{ id: 'self', name: 'You', screenId: 'local-stream' }, { id: 'partner', name: 'Partner', screenId: 'remote-stream' }];
  await page.evaluate(participants => { window.fixtureRoster = { participants, scale: 100, showNames: true }; window.fixtureEmit('voice-hud:update', window.fixtureRoster); }, participants);
  await page.locator('.voice-hud__stream').nth(1).waitFor();
  await page.evaluate(() => {
    const canvas = document.createElement('canvas'); canvas.width = 320; canvas.height = 180;
    const context = canvas.getContext('2d'); context.fillStyle = '#50b080'; context.fillRect(0, 0, 320, 180);
    for (const p of window.fixtureRoster.participants) window.fixtureEmit('voice-hud:frame', { participantId: p.id, streamId: p.screenId, image: canvas.toDataURL('image/jpeg') });
    window.fixtureTimer = setInterval(() => window.fixtureEmit('voice-hud:update', window.fixtureRoster), 1000);
  });
  await page.waitForFunction(() => document.querySelectorAll('.voice-hud__stream img').length === 2);
  const geometry = await page.locator('.voice-hud__participant-group').evaluateAll(groups => groups.map(group => {
    const avatar = group.querySelector('.voice-hud__participant').getBoundingClientRect(), stream = group.querySelector('.voice-hud__stream').getBoundingClientRect();
    return { below: stream.top >= avatar.bottom, right: stream.right, bottom: stream.bottom };
  }));
  assert(geometry.every(g => g.below && g.right <= 600 && g.bottom <= 600));
  await page.screenshot({ path: 'audits/streaming-overlay-smoke.png' });
  await page.waitForFunction(() => document.querySelectorAll('.voice-hud__stream img').length === 0, null, { timeout: 6000 });
  assert.equal(await page.locator('.voice-hud__stream').count(), 2);
  await page.evaluate(() => {
    window.fixtureRoster.participants[0].screenId = null;
    window.fixtureEmit('voice-hud:update', window.fixtureRoster);
    window.fixtureEmit('voice-hud:frame', { participantId: 'self', streamId: 'local-stream', image: 'data:image/jpeg;base64,old' });
  });
  await page.waitForFunction(() => document.querySelectorAll('.voice-hud__stream').length === 1);
  await page.evaluate(() => { clearInterval(window.fixtureTimer); window.fixtureEmit('voice-hud:update', { participants: [], scale: 100, showNames: false }); });
  await page.waitForFunction(() => document.querySelectorAll('.voice-hud__participant-group').length === 0);
  await page.evaluate(() => {
    window.fixtureRoster = { participants: [{ id: 'partner', name: 'Partner', message: { id: 'first', text: 'Latest message', expiresAt: Date.now() + 10_000 } }], scale: 100, showNames: true };
    window.fixtureEmit('voice-hud:update', window.fixtureRoster);
    window.fixtureTimer = setInterval(() => window.fixtureEmit('voice-hud:update', window.fixtureRoster), 1000);
  });
  await page.getByRole('status').filter({ hasText: 'Latest message' }).waitFor();
  const bubbleStyle = await page.locator('.voice-hud__message').evaluate(node => { const style = getComputedStyle(node); return { background: style.backgroundColor, border: style.borderTopWidth, font: style.fontSize }; });
  assert.deepEqual(bubbleStyle, { background: 'rgb(23, 23, 23)', border: '0px', font: '15px' });
  await page.waitForTimeout(650);
  await page.evaluate(() => { window.fixtureFirstBubble = document.querySelector('.voice-hud__message'); });
  await page.waitForTimeout(1100);
  assert(await page.evaluate(() => window.fixtureFirstBubble === document.querySelector('.voice-hud__message')), 'roster replay must not restart the bubble');
  await page.evaluate(() => {
    window.fixtureRoster.participants[0].message = { id: 'replacement', text: 'Replacement message', expiresAt: Date.now() + 10_000 };
    window.fixtureEmit('voice-hud:update', window.fixtureRoster);
  });
  await page.getByRole('status').filter({ hasText: 'Replacement message' }).waitFor();
  assert(await page.evaluate(() => window.fixtureFirstBubble !== document.querySelector('.voice-hud__message')));
  await page.waitForFunction(() => document.querySelectorAll('.voice-hud__message').length === 0, null, { timeout: 12000 });
  await page.evaluate(() => clearInterval(window.fixtureTimer));
  assert.deepEqual(errors, []);
  console.log('PASS: built HUD places both streams below their owners, expires stale frames, and removes stopped/ended UI');
  console.log('PASS: latest-message bubbles replace older messages, survive roster replay, and expire after ten seconds');
} finally {
  await browser?.close();
  await new Promise(resolve => host.close(resolve));
}
