// Production-function regressions, with no accounts, capture, or live requests.
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { runInNewContext } from 'node:vm';
import assert from 'node:assert/strict';

const load = path => readFileSync(new URL(path, import.meta.url), 'utf8');
const compile = (source, globals) => runInNewContext(stripTypeScriptTypes(source), globals);
const layout = compile(load('../src/lib/screen-share-layout.ts').replace(/export /g, '') + '\nshouldFillScreenPreview', {});
assert(layout(2560, 1080)); assert(!layout(1920, 1080)); assert(!layout(1080, 1920)); assert(!layout(0, 0));
const between = (source, start, end) => {
  const offset = source.indexOf(start);
  assert(offset >= 0, start);
  const finish = source.indexOf(end, offset + start.length);
  assert(finish >= 0, end);
  return source.slice(offset, finish);
};
const deferred = () => { let resolve; const promise = new Promise(r => { resolve = r; }); return { promise, resolve }; };
const client = load('../src/lib/cloudflare-realtime.ts').replace(/^import .*;\r?\n/gm, '').replace(/export /g, '');

function transportFixture({ trackError = false, connected = true } = {}) {
  const requests = [], peers = [], announcements = [];
  class Stream {
    tracks = [];
    addTrack(track) { this.tracks.push(track); }
    getTracks() { return this.tracks; }
  }
  class Peer extends EventTarget {
    constructor() { super(); this.connectionState = 'new'; this.iceGatheringState = 'complete'; this.transceivers = []; peers.push(this); }
    addTransceiver(track) {
      const transceiver = { mid: null, sender: { track, getParameters: () => ({ encodings: [{}] }), setParameters: async () => {} } };
      this.transceivers.push(transceiver);
      return transceiver;
    }
    async createOffer() { assert(this.transceivers.length, 'must not create an empty SDP offer'); return { type: 'offer', sdp: 'm=video\r\na=ice-ufrag:test' }; }
    async createAnswer() { assert(this.remoteDescription); return { type: 'answer', sdp: 'm=video\r\na=ice-ufrag:test' }; }
    async setLocalDescription(value) {
      this.localDescription = { ...value, toJSON: () => value };
      this.transceivers.forEach((t, i) => { t.mid = String(i); });
    }
    async setRemoteDescription(value) {
      this.remoteDescription = value;
      if (value.type === 'offer') {
        this.ontrack?.({ track: { kind: 'audio', id: 'audio-remote' } });
        assert.equal(announcements.length, 0, 'audio alone must not create a screen');
        this.ontrack?.({ track: { kind: 'video', id: 'video-remote' } });
      }
      if (connected) this.connectionState = 'connected';
    }
    close() { this.connectionState = 'closed'; this.dispatchEvent(new Event('connectionstatechange')); }
  }
  const globals = { RTCPeerConnection: Peer, MediaStream: Stream, DOMException, window: { setTimeout, clearTimeout }, SCREEN_SHARE_MAX_BITRATE: 1_000_000, recordVoiceEvent() {},
    supabase: { functions: { invoke: async (_, { body }) => {
      requests.push(body);
      if (body.action === 'create') {
        assert.equal(body.sessionDescription, undefined, 'sessions/new must not receive empty SDP');
        return { data: { sessionId: 'provider-session' } };
      }
      if (body.action === 'publish') {
        assert.equal(body.tracks.length, 2);
        assert.equal(body.tracks[0].mid, '0');
        assert.match(body.sessionDescription.sdp, /ice-ufrag/);
        return { data: { sessionDescription: { type: 'answer', sdp: 'answer' }, ...(trackError ? { tracks: [{ errorCode: 'invalid_track' }] } : {}) } };
      }
      if (body.action === 'subscribe') return { data: { requiresImmediateRenegotiation: true, sessionDescription: { type: 'offer', sdp: 'remote offer' } } };
      assert.equal(body.action, 'renegotiate');
      assert.equal(body.sessionDescription.type, 'answer');
      return { data: {} };
    } } },
  };
  const factories = compile(client + '\n({createCloudflareScreenPublisher, createCloudflareScreenSubscriber})', globals);
  const stream = { getTracks: () => [{ kind: 'video', id: 'video-track' }, { kind: 'audio', id: 'audio-track' }] };
  return { ...factories, stream, requests, peers, announcements };
}
{
  const f = transportFixture();
  const result = await f.createCloudflareScreenPublisher('room', f.stream, 'voice');
  assert.deepEqual(f.requests.map(r => r.action), ['create', 'publish']);
  assert.equal(result.connection.connectionState, 'connected');
  assert.equal(result.trackNames.length, 2);
  console.log('PASS: publisher negotiates video/audio after an SDP-less session create');
}
{
  const f = transportFixture();
  await f.createCloudflareScreenSubscriber('room', 'publisher', ['video', 'audio'], stream => f.announcements.push(stream), 'voice');
  assert.deepEqual(f.requests.map(r => r.action), ['create', 'subscribe', 'renegotiate']);
  assert.equal(f.announcements.length, 1);
  assert.equal(f.announcements[0].getTracks().length, 2);
  console.log('PASS: subscriber answers the SFU offer and combines screen audio/video');
}
{
  const f = transportFixture({ trackError: true });
  await assert.rejects(f.createCloudflareScreenPublisher('room', f.stream, 'voice'), /screen tracks/);
  assert.equal(f.peers[0].connectionState, 'closed');
  console.log('PASS: per-track provider errors fail publication and close transport');
}
{
  const f = transportFixture({ connected: false }), controller = new AbortController();
  const task = f.createCloudflareScreenPublisher('room', f.stream, 'voice', [], controller.signal);
  // Allow it to enter the connection wait, then cancel rather than timing out.
  await new Promise(resolve => setTimeout(resolve, 10));
  controller.abort();
  await assert.rejects(task, /canceled/);
  assert.equal(f.peers[0].connectionState, 'closed');
  console.log('PASS: stopping during the media connection wait cancels immediately');
}

const voice = load('../src/stores/voice.ts');
{
  let messages = {}, listener, cleanup;
  let userId = 'self', sessionId = 'seat';
  const hook = load('../src/features/voice-hud/use-voice-hud-bridge.ts');
  const effect = between(hook, '  useEffect(() => {\n    setMessages({});', '\n  useEffect(() => {\n    const expiries');
  const globals = { joined: true, isTauri: true, conversationId: 'room', sessionId: 'seat', userId: 'self', messageScope: 'self:room:seat',
    useEffect: callback => { cleanup = callback(); }, setMessages: patch => { messages = typeof patch === 'function' ? patch(messages) : patch; },
    subscribeToLiveChatMessages: callback => { listener = callback; return () => { listener = null; }; },
    useAuth: { getState: () => ({ userId }) }, useVoice: { getState: () => ({ sessionId, participants: { room: [{ user_id: 'partner' }] } }) } };
  compile(effect, globals);
  const message = { id: 'first', sender_id: 'partner', conversation_id: 'room', message_kind: 'chat', content: 'Hello' };
  listener({ kind: 'insert', message: { ...message, conversation_id: 'other' } }); assert.equal(Object.keys(messages).length, 0);
  listener({ kind: 'insert', message }); assert.equal(messages.partner.text, 'Hello');
  const expiry = messages.partner.expiresAt; assert(Math.abs(expiry - Date.now() - 10000) < 100);
  listener({ kind: 'insert', message }); assert.equal(messages.partner.expiresAt, expiry);
  listener({ kind: 'update', message: { ...message, content: 'Edited' } }); assert.equal(messages.partner.text, 'Edited'); assert.equal(messages.partner.expiresAt, expiry);
  listener({ kind: 'insert', message: { ...message, id: 'second', content: 'Latest' } }); assert.equal(messages.partner.id, 'second');
  listener({ kind: 'update', message: { ...message, id: 'second', deleted_at: 'now' } }); assert.equal(Object.keys(messages).length, 0);
  sessionId = 'new-seat'; listener({ kind: 'insert', message: { ...message, id: 'third' } }); assert.equal(Object.keys(messages).length, 0);
  cleanup(); assert.equal(listener, null);
  console.log('PASS: live HUD messages isolate conversation/session, replace, deduplicate, edit/delete, and retain ten-second expiry');
}
{
  const globals = { remoteScreenRevision: null };
  const accept = compile(between(voice, 'function acceptScreenRevision(', '\nfunction isCurrentVoiceSignal') + '\nacceptScreenRevision', globals);
  assert(accept({ fromSessionId: 'seat', screenRevision: 1 }));
  assert(accept({ fromSessionId: 'seat', screenRevision: 2 }));
  assert(!accept({ fromSessionId: 'seat', screenRevision: 1 }));
  assert(!accept({ fromSessionId: 'seat', screenRevision: -1 }));
  assert(accept({ fromSessionId: 'replacement-seat', screenRevision: 1 }));
  console.log('PASS: delayed publish/stop signals cannot overwrite newer stream state');
}
{
  const pending = deferred(), calls = [];
  let state = { sessionId: 'seat', activeConversationId: 'room', sharingScreen: true, status: 'connected' };
  const globals = { leaseRequest: null, AbortSignal, recordVoiceEvent() {}, sameVoiceSession: () => true,
    useVoice: { getState: () => state }, supabase: { rpc: (_, body) => ({ abortSignal: () => {
      calls.push(body.p_sharing_screen);
      return calls.length === 1 ? pending.promise : Promise.resolve({ data: { status: 'ok' } });
    } }) } };
  const heartbeat = compile(between(voice, 'async function sendHeartbeat()', '\nasync function recoverMembership') + '\nsendHeartbeat', globals);
  const first = heartbeat(); state = { ...state, sharingScreen: false }; const stopped = heartbeat();
  pending.resolve({ data: { status: 'ok' } });
  await Promise.all([first, stopped]);
  assert.deepEqual(calls, [true, false]);
  console.log('PASS: stream stop is persisted after an older in-flight heartbeat');
}
{
  let state = { sessionId: 'seat', activeConversationId: 'room', remoteScreenStream: null };
  const globals = { screenSubscribeAttempt: 0, screenSubscriberAbort: null, subscriberRenewalTimer: null, screenSubscriber: null,
    remoteScreen: {}, AbortController, activeIceServers: [], sameVoiceSession: () => true, recordVoiceEvent() {}, scheduleScreenRecovery() {},
    useVoice: { getState: () => state, setState: patch => { state = { ...state, ...patch }; } },
    createCloudflareScreenSubscriber: async (_, __, ___, onTrack) => {
      onTrack({ getVideoTracks: () => [{ addEventListener() {} }] });
      throw Error('renegotiation failed after ontrack');
    } };
  const subscribe = compile(between(voice, 'async function subscribeRemoteScreen(', '\nasync function recoverScreens') + '\nsubscribeRemoteScreen', globals);
  await subscribe(globals.remoteScreen);
  assert.equal(state.remoteScreenStream, null);
  console.log('PASS: failed subscription clears media exposed before renegotiation failed');
}
{
  let stopped = 0, canceled = 0, state;
  const track = { onended() {}, stop() { stopped++; } };
  const globals = { screenPublishAttempt: 1, screenPublisherAbort: { abort() { canceled++; } }, publisherRenewalTimer: null,
    screenPublisher: { close() {} }, publishedScreen: {}, localScreenTrack: track, localScreenStream: { getTracks: () => [track] },
    screenAudioCleanup() {}, sendScreenStopped() {}, useVoice: { setState: value => { state = value; } } };
  const stop = compile(between(voice, 'async function stopLocalScreen(', '\nfunction scheduleConnectionFailure') + '\nstopLocalScreen', globals);
  await stop(false);
  assert.equal(stopped, 1); assert.equal(canceled, 1); assert.equal(track.onended, null);
  assert.equal(state.sharingScreen, false); assert.equal(state.screenShareStarting, false); assert.equal(state.localScreenStream, null);
  console.log('PASS: stop ends capture and clears both streaming and starting controls');
}
{
  const callbacks = [], events = [], videos = [];
  const stream = { id: 'active', getVideoTracks: () => [{ readyState: 'live', muted: false, stop() { throw Error('overlay must not stop owned tracks'); } }] };
  const globals = { isTauri: true, shouldFillScreenPreview: layout, setTimeout: callback => { callbacks.push(callback); return callbacks.length; }, clearTimeout() {},
    emitTo: async (_, event, payload) => events.push({ event, payload }), MediaStream: class { constructor(tracks) { this.tracks = tracks; } },
    document: { createElement: kind => {
      if (kind === 'video') { const video = { readyState: 2, currentTime: 1, videoWidth: 1280, videoHeight: 720, play: async () => {}, pause() {} }; videos.push(video); return video; }
      return { getContext: () => ({ fillRect() {}, drawImage() {} }), toDataURL: () => 'data:image/jpeg;base64,frame' };
    } } };
  const source = load('../src/lib/voice-hud-stream.ts').replace(/^import .*;\r?\n/gm, '').replace(/export /g, '');
  const relay = compile(source + '\nrelayVoiceHudStream', globals);
  const stop = relay('owner', stream);
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(events.length, 1); assert.equal(events[0].payload.streamId, 'active');
  assert.equal(videos[0].muted, true);
  stop(); await callbacks[0]();
  assert.equal(events.length, 1); assert.equal(videos[0].srcObject, null);
  console.log('PASS: overlay relays owner-tagged video only and stops without stopping call media');
}

{
  const source = load('../supabase/functions/cloudflare-realtime/index.ts').split('export default')[0].replace(/^import .*;\r?\n/gm, '');
  const requests = [], writes = [];
  const user = '11111111-1111-4111-8111-111111111111', voiceSessionId = '22222222-2222-4222-8222-222222222222', conversationId = '33333333-3333-4333-8333-333333333333';
  let failTracks = false, failPersistence = false, participantPresent = true;
  const admin = { schema: () => { throw Error('private schema must not be queried over PostgREST'); }, rpc: async (name, args) => {
    if (name === 'read_cloudflare_screen_session') return { data: { session_id: args.p_session_id, owner_id: user, voice_session_id: voiceSessionId } };
    assert.equal(name, 'save_cloudflare_screen_session');
    writes.push(args);
    return { data: !failPersistence, error: failPersistence ? { code: 'test_failure' } : null };
  }, from: table => {
    const query = {
      select() { return query; }, eq() { return query; }, gt() { return query; }, lte() { return query; }, delete() { return query; },
      maybeSingle: async () => ({ data: table === 'voice_participants' ? participantPresent ? { session_id: voiceSessionId } : null : table === 'conversations' ? { user1_id: user, user2_id: 'partner' } : { session_id: 'provider-session' } }),
      upsert: async data => { writes.push(data); return { error: null }; },
      update: data => { writes.push(data); return { eq: async () => ({ error: failPersistence ? { code: 'test_failure' } : null }) }; },
    };
    return query;
  } };
  const globals = { Request, Response, AbortSignal, console: { error() {} }, withSupabase: (_, handler) => handler, allowRequest: async () => true,
    Deno: { env: { get: () => 'test-only-placeholder' } },
    fetch: async (url, options) => {
      requests.push({ url, ...options, parsed: options.body === undefined ? undefined : JSON.parse(options.body) });
      if (url.endsWith('/sessions/new') && options.body === undefined) {
        const request = new Request(url, options);
        assert.equal(await request.text(), '', 'session creation must send no body, not {}');
        assert.equal(request.headers.get('content-type'), null, 'bodyless creates must not select the JSON validation schema');
      }
      return Response.json(url.endsWith('/sessions/new') ? { sessionId: 'provider-session' } : failTracks ? { tracks: [{ errorCode: 'invalid_track', errorDescription: 'synthetic failure' }] } : { tracks: [{}] });
    } };
  const handler = compile(source + '\nhandler', globals);
  const call = body => handler(new Request('https://test.invalid', { method: 'POST', body: JSON.stringify({ conversationId, voiceSessionId, ...body }) }), { userClaims: { id: user }, supabaseAdmin: admin });
  let response = await call({ action: 'create' }); assert.equal(response.status, 200);
  assert.equal(requests[0].body, undefined); assert.equal(requests[0].headers['Content-Type'], undefined);
  assert.equal(writes[0].p_voice_session_id, voiceSessionId);
  response = await call({ action: 'create', sessionDescription: { type: 'offer', sdp: 'valid legacy SDP' } }); assert.equal(response.status, 200);
  assert.equal(requests[1].parsed.sessionDescription.sdp, 'valid legacy SDP');
  participantPresent = false; response = await call({ action: 'create' }); assert.equal(response.status, 403); assert.equal(requests.length, 2); participantPresent = true;
  const publication = { action: 'publish', sessionId: 'provider-session', sessionDescription: { type: 'offer', sdp: 'media SDP' }, tracks: [{ mid: '0', trackName: 'video-track' }] };
  failTracks = true; response = await call(publication); assert.equal(response.status, 502); assert.equal(writes.length, 2);
  failTracks = false; failPersistence = true; response = await call(publication); assert.equal(response.status, 500);
  console.log('PASS: broker supports new/legacy session creation, retains membership authorization, and rejects track/persistence failures');
}
