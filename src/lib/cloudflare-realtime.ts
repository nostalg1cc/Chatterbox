import { supabase } from "@/lib/supabase";
import { SCREEN_SHARE_MAX_BITRATE } from "@/lib/voice-media";
import { recordVoiceEvent } from "@/lib/voice-diagnostics";

type Sdp = { type: RTCSdpType; sdp: string };
type CallsResponse = { sessionId?: string; sessionDescription?: Sdp; requiresImmediateRenegotiation?: boolean; tracks?: { errorCode?: string; errorDescription?: string }[] };

const CLOUDFLARE_ICE_SERVERS: RTCIceServer[] = [{ urls: "stun:stun.cloudflare.com:3478" }];
const CONNECTION_TIMEOUT_MS = 8_000;

async function call(conversationId: string, voiceSessionId: string, body: Record<string, unknown>, signal?: AbortSignal): Promise<CallsResponse> {
  const { data, error } = await supabase.functions.invoke("cloudflare-realtime", { body: { conversationId, voiceSessionId, ...body }, timeout: 10_000, signal });
  signal?.throwIfAborted();
  if (error || !data || typeof data !== "object") {
    recordVoiceEvent("screen_request_failed", { action: String(body.action), reason: error?.name ?? "invalid_response" });
    throw new Error(`Screen sharing could not ${body.action === "subscribe" ? "connect to your partner" : "connect to Cloudflare"}. Please try again.`);
  }
  if (data.errorCode || data.tracks?.some((track: { errorCode?: string }) => track.errorCode)) {
    recordVoiceEvent("screen_request_failed", { action: String(body.action), reason: "provider_track_error" });
    throw new Error("Cloudflare could not prepare the screen tracks.");
  }
  return data as CallsResponse;
}

async function waitForIceComplete(connection: RTCPeerConnection, signal?: AbortSignal): Promise<void> {
  signal?.throwIfAborted();
  if (connection.iceGatheringState === "complete") return;
  await new Promise<void>((resolve, reject) => {
    const timer = window.setTimeout(done, 3_000);
    function done() { window.clearTimeout(timer); connection.removeEventListener("icegatheringstatechange", ready); signal?.removeEventListener("abort", aborted); if (signal?.aborted) reject(signal.reason); else resolve(); }
    function aborted() { done(); }
    function ready() { if (connection.iceGatheringState === "complete") done(); }
    connection.addEventListener("icegatheringstatechange", ready);
    signal?.addEventListener("abort", aborted, { once: true });
  });
}

async function waitForConnection(connection: RTCPeerConnection, signal?: AbortSignal): Promise<void> {
  signal?.throwIfAborted();
  if (connection.connectionState === "connected") return;
  await new Promise<void>((resolve, reject) => {
    const timer = window.setTimeout(() => done(new Error("Cloudflare media connection timed out.")), CONNECTION_TIMEOUT_MS);
    function done(error?: Error) {
      window.clearTimeout(timer);
      connection.removeEventListener("connectionstatechange", stateChanged);
      signal?.removeEventListener("abort", aborted);
      if (error) reject(error); else resolve();
    }
    function stateChanged() {
      if (signal?.aborted) { aborted(); return; }
      if (connection.connectionState === "connected") done();
      if (connection.connectionState === "failed" || connection.connectionState === "closed") done(new Error("Cloudflare media connection failed."));
    }
    function aborted() { done(new DOMException("Screen sharing canceled.", "AbortError")); }
    connection.addEventListener("connectionstatechange", stateChanged);
    signal?.addEventListener("abort", aborted, { once: true });
    stateChanged();
  });
}

// Publishes every track in the stream (video, and audio when the OS/browser
// captured any) in a single negotiation round, not just the video track -
// screen-share audio was previously captured fine but silently dropped
// here, since only the video transceiver was ever added. This is also now
// the *only* path screen share ever goes out on - it used to also ride
// along on the main voice call's own peerConnection as a fallback, but that
// meant screen video/audio competed for that connection's bandwidth with
// live voice audio (and screen audio was indistinguishable from voice audio
// on the receiving end). If this fails, screen sharing just isn't available
// this session - see startScreenShare in stores/voice.ts.
export async function createCloudflareScreenPublisher(conversationId: string, stream: MediaStream, voiceSessionId: string, iceServers = CLOUDFLARE_ICE_SERVERS, signal?: AbortSignal) {
  const connection = new RTCPeerConnection({ iceServers, bundlePolicy: "max-bundle" });
  const cancel = () => connection.close();
  signal?.addEventListener("abort", cancel, { once: true });
  try {
  signal?.throwIfAborted();
  // Cloudflare's documented lifecycle is create session -> publish tracks ->
  // establish media. Waiting for connection before publication can deadlock a
  // session which has no media section yet.
  const created = await call(conversationId, voiceSessionId, { action: "create" }, signal);
  if (!created.sessionId) throw new Error("Cloudflare did not create a screen session.");

  const tracks = stream.getTracks();
  const transceivers = tracks.map((track) => connection.addTransceiver(track, { direction: "sendonly", streams: [stream] }));
  // Without this, WebRTC's default degradationPreference ("balanced")
  // sacrifices framerate to hold resolution under any bandwidth/CPU
  // pressure the congestion controller perceives - exactly what a mostly
  // still screen-share signal (occasional big deltas, otherwise near-idle)
  // tends to trigger, producing the "near-frozen" choppiness.
  for (const transceiver of transceivers) {
    if (transceiver.sender.track?.kind !== "video") continue;
    const parameters = transceiver.sender.getParameters();
    if (parameters.encodings.length > 0) {
      parameters.degradationPreference = "maintain-framerate";
      parameters.encodings[0].maxBitrate = SCREEN_SHARE_MAX_BITRATE;
      parameters.encodings[0].maxFramerate = 30;
      await transceiver.sender.setParameters(parameters).catch(() => undefined);
    }
  }
  await connection.setLocalDescription(await connection.createOffer());
  await waitForIceComplete(connection, signal);
  const trackRefs = transceivers.map((transceiver, index) => {
    if (transceiver.mid === null) throw new Error("Cloudflare could not prepare the screen track.");
    return { mid: transceiver.mid, trackName: tracks[index].id };
  });
  const published = await call(conversationId, voiceSessionId, { action: "publish", sessionId: created.sessionId, tracks: trackRefs, sessionDescription: connection.localDescription?.toJSON() }, signal);
  if (!published.sessionDescription || published.sessionDescription.type !== "answer") throw new Error("Cloudflare did not answer the screen publication.");
  await connection.setRemoteDescription(published.sessionDescription);
  await waitForConnection(connection, signal);
  return { connection, sessionId: created.sessionId, trackNames: trackRefs.map((ref) => ref.trackName) };
  } catch (error) { connection.close(); throw error; }
  finally { signal?.removeEventListener("abort", cancel); }
}

export async function createCloudflareScreenSubscriber(conversationId: string, remoteSessionId: string, trackNames: string[], onTrack: (stream: MediaStream) => void, voiceSessionId: string, iceServers = CLOUDFLARE_ICE_SERVERS, signal?: AbortSignal) {
  const connection = new RTCPeerConnection({ iceServers, bundlePolicy: "max-bundle" });
  const cancel = () => connection.close();
  signal?.addEventListener("abort", cancel, { once: true });
  try {
  signal?.throwIfAborted();
  // Every published track lands in its own ontrack firing - accumulate them
  // into one live MediaStream and hand it off once, rather than resetting
  // whatever <video>/<audio> element consumes it on each additional track.
  const combined = new MediaStream();
  let announced = false;
  connection.ontrack = (event) => {
    combined.addTrack(event.track);
    // Do not expose an audio-only stream as a usable screen preview.
    if (!announced && event.track.kind === "video") {
      announced = true;
      onTrack(combined);
    }
  };
  const created = await call(conversationId, voiceSessionId, { action: "create" }, signal);
  if (!created.sessionId) throw new Error("Cloudflare did not create a viewing session.");

  const subscribed = await call(conversationId, voiceSessionId, { action: "subscribe", sessionId: created.sessionId, remoteSessionId, trackNames }, signal);
  if (!subscribed.sessionDescription || subscribed.sessionDescription.type !== "offer") throw new Error("Cloudflare did not offer the shared screen.");
  await connection.setRemoteDescription(subscribed.sessionDescription);
  await connection.setLocalDescription(await connection.createAnswer());
  await waitForIceComplete(connection, signal);
  await call(conversationId, voiceSessionId, { action: "renegotiate", sessionId: created.sessionId, sessionDescription: connection.localDescription?.toJSON() }, signal);
  await waitForConnection(connection, signal);
  return connection;
  } catch (error) { connection.close(); throw error; }
  finally { signal?.removeEventListener("abort", cancel); }
}
