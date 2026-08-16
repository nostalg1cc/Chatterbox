import { supabase } from "@/lib/supabase";

type Sdp = { type: RTCSdpType; sdp: string };
type CallsResponse = { sessionId?: string; sessionDescription?: Sdp; requiresImmediateRenegotiation?: boolean };

const CLOUDFLARE_ICE_SERVERS: RTCIceServer[] = [{ urls: "stun:stun.cloudflare.com:3478" }];
const CONNECTION_TIMEOUT_MS = 8_000;

async function call(conversationId: string, body: Record<string, unknown>): Promise<CallsResponse> {
  const { data, error } = await supabase.functions.invoke("cloudflare-realtime", { body: { conversationId, ...body } });
  if (error || !data || typeof data !== "object") throw new Error("Cloudflare screen sharing is unavailable.");
  return data as CallsResponse;
}

async function waitForIceComplete(connection: RTCPeerConnection): Promise<void> {
  if (connection.iceGatheringState === "complete") return;
  await new Promise<void>((resolve) => {
    const timer = window.setTimeout(done, 3_000);
    function done() { window.clearTimeout(timer); connection.removeEventListener("icegatheringstatechange", ready); resolve(); }
    function ready() { if (connection.iceGatheringState === "complete") done(); }
    connection.addEventListener("icegatheringstatechange", ready);
  });
}

async function waitForConnection(connection: RTCPeerConnection): Promise<void> {
  if (connection.connectionState === "connected") return;
  await new Promise<void>((resolve, reject) => {
    const timer = window.setTimeout(() => done(new Error("Cloudflare media connection timed out.")), CONNECTION_TIMEOUT_MS);
    function done(error?: Error) {
      window.clearTimeout(timer);
      connection.removeEventListener("connectionstatechange", stateChanged);
      if (error) reject(error); else resolve();
    }
    function stateChanged() {
      if (connection.connectionState === "connected") done();
      if (connection.connectionState === "failed" || connection.connectionState === "closed") done(new Error("Cloudflare media connection failed."));
    }
    connection.addEventListener("connectionstatechange", stateChanged);
  });
}

// Publishes every track in the stream (video, and audio when the OS/browser
// captured any) in a single negotiation round, not just the video track -
// screen-share audio was previously captured fine but silently dropped
// here, since only the video transceiver was ever added.
export async function createCloudflareScreenPublisher(conversationId: string, stream: MediaStream) {
  const connection = new RTCPeerConnection({ iceServers: CLOUDFLARE_ICE_SERVERS });
  // Cloudflare's documented lifecycle is create session -> publish tracks ->
  // establish media. Waiting for connection before publication can deadlock a
  // session which has no media section yet.
  await connection.setLocalDescription(await connection.createOffer());
  await waitForIceComplete(connection);
  const created = await call(conversationId, { action: "create", sessionDescription: connection.localDescription?.toJSON() });
  if (!created.sessionId || !created.sessionDescription) throw new Error("Cloudflare did not create a screen session.");
  await connection.setRemoteDescription(created.sessionDescription);

  const tracks = stream.getTracks();
  const transceivers = tracks.map((track) => connection.addTransceiver(track, { direction: "sendonly", streams: [stream] }));
  // Without this, WebRTC's default degradationPreference ("balanced")
  // sacrifices framerate to hold resolution under any bandwidth/CPU
  // pressure the congestion controller perceives - exactly what a mostly
  // still screen-share signal (occasional big deltas, otherwise near-idle)
  // tends to trigger, producing the "near-frozen" choppiness. The direct
  // P2P fallback (addScreenTracks in voice.ts) already sets this; the
  // Cloudflare relay path - the one actually used whenever it connects -
  // never did.
  for (const transceiver of transceivers) {
    if (transceiver.sender.track?.kind !== "video") continue;
    const parameters = transceiver.sender.getParameters();
    if (parameters.encodings.length > 0) {
      parameters.degradationPreference = "maintain-framerate";
      parameters.encodings[0].maxBitrate = 5_000_000;
      parameters.encodings[0].maxFramerate = 30;
      await transceiver.sender.setParameters(parameters).catch(() => undefined);
    }
  }
  await connection.setLocalDescription(await connection.createOffer());
  await waitForIceComplete(connection);
  const trackRefs = transceivers.map((transceiver, index) => {
    if (!transceiver.mid) throw new Error("Cloudflare could not prepare the screen track.");
    return { mid: transceiver.mid, trackName: tracks[index].id };
  });
  const published = await call(conversationId, { action: "publish", sessionId: created.sessionId, tracks: trackRefs, sessionDescription: connection.localDescription?.toJSON() });
  if (published.sessionDescription) await connection.setRemoteDescription(published.sessionDescription);
  await waitForConnection(connection);
  return { connection, sessionId: created.sessionId, trackNames: trackRefs.map((ref) => ref.trackName) };
}

export async function createCloudflareScreenSubscriber(conversationId: string, remoteSessionId: string, trackNames: string[], onTrack: (stream: MediaStream) => void) {
  const connection = new RTCPeerConnection({ iceServers: CLOUDFLARE_ICE_SERVERS });
  // Every published track lands in its own ontrack firing - accumulate them
  // into one live MediaStream and hand it off once, rather than resetting
  // whatever <video>/<audio> element consumes it on each additional track.
  const combined = new MediaStream();
  let announced = false;
  connection.ontrack = (event) => {
    combined.addTrack(event.track);
    if (!announced) {
      announced = true;
      onTrack(combined);
    }
  };
  await connection.setLocalDescription(await connection.createOffer());
  await waitForIceComplete(connection);
  const created = await call(conversationId, { action: "create", sessionDescription: connection.localDescription?.toJSON() });
  if (!created.sessionId || !created.sessionDescription) throw new Error("Cloudflare did not create a viewing session.");
  await connection.setRemoteDescription(created.sessionDescription);

  const subscribed = await call(conversationId, { action: "subscribe", sessionId: created.sessionId, remoteSessionId, trackNames });
  if (subscribed.requiresImmediateRenegotiation && subscribed.sessionDescription) {
    await connection.setRemoteDescription(subscribed.sessionDescription);
    await connection.setLocalDescription(await connection.createAnswer());
    await call(conversationId, { action: "renegotiate", sessionId: created.sessionId, sessionDescription: connection.localDescription?.toJSON() });
  }
  await waitForConnection(connection);
  return connection;
}