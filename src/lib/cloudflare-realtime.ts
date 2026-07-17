import { supabase } from "@/lib/supabase";

type Sdp = { type: RTCSdpType; sdp: string };
type CallsResponse = { sessionId?: string; sessionDescription?: Sdp; requiresImmediateRenegotiation?: boolean };

async function call(conversationId: string, body: Record<string, unknown>): Promise<CallsResponse> {
  const { data, error } = await supabase.functions.invoke("cloudflare-realtime", { body: { conversationId, ...body } });
  if (error || !data || typeof data !== "object") throw new Error("Cloudflare screen sharing is unavailable.");
  return data as CallsResponse;
}

async function waitForIceComplete(connection: RTCPeerConnection): Promise<void> {
  if (connection.iceGatheringState === "complete") return;
  await new Promise<void>((resolve) => {
    const timer = window.setTimeout(done, 1200);
    function done() { window.clearTimeout(timer); connection.removeEventListener("icegatheringstatechange", ready); resolve(); }
    function ready() { if (connection.iceGatheringState === "complete") done(); }
    connection.addEventListener("icegatheringstatechange", ready);
  });
}

export async function createCloudflareScreenPublisher(conversationId: string, stream: MediaStream, track: MediaStreamTrack) {
  const connection = new RTCPeerConnection();
  const transceiver = connection.addTransceiver(track, { direction: "sendonly", streams: [stream] });
  await connection.setLocalDescription(await connection.createOffer());
  await waitForIceComplete(connection);
  const created = await call(conversationId, { action: "create", sessionDescription: connection.localDescription?.toJSON() });
  if (!created.sessionId || !created.sessionDescription) throw new Error("Cloudflare did not create a screen session.");
  await connection.setRemoteDescription(created.sessionDescription);
  await connection.setLocalDescription(await connection.createOffer());
  await waitForIceComplete(connection);
  const published = await call(conversationId, { action: "publish", sessionId: created.sessionId, mid: transceiver.mid, trackName: track.id, sessionDescription: connection.localDescription?.toJSON() });
  if (published.sessionDescription) await connection.setRemoteDescription(published.sessionDescription);
  return { connection, sessionId: created.sessionId, trackName: track.id };
}

export async function createCloudflareScreenSubscriber(conversationId: string, remoteSessionId: string, trackName: string, onTrack: (stream: MediaStream) => void) {
  const connection = new RTCPeerConnection();
  connection.ontrack = (event) => onTrack(event.streams[0] ?? new MediaStream([event.track]));
  await connection.setLocalDescription(await connection.createOffer());
  await waitForIceComplete(connection);
  const created = await call(conversationId, { action: "create", sessionDescription: connection.localDescription?.toJSON() });
  if (!created.sessionId || !created.sessionDescription) throw new Error("Cloudflare did not create a viewing session.");
  await connection.setRemoteDescription(created.sessionDescription);
  const subscribed = await call(conversationId, { action: "subscribe", sessionId: created.sessionId, remoteSessionId, trackName });
  if (subscribed.requiresImmediateRenegotiation && subscribed.sessionDescription) {
    await connection.setRemoteDescription(subscribed.sessionDescription);
    await connection.setLocalDescription(await connection.createAnswer());
    await call(conversationId, { action: "renegotiate", sessionId: created.sessionId, sessionDescription: connection.localDescription?.toJSON() });
  }
  return connection;
}