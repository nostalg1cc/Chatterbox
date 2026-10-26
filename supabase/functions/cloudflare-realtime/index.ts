import "@supabase/functions-js/edge-runtime.d.ts";
import { withSupabase } from "@supabase/server";
import { allowRequest } from "../_shared/quota.ts";

const corsHeaders = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Headers": "authorization, apikey, content-type, x-client-info",
  "Access-Control-Allow-Methods": "POST, OPTIONS",
};

const json = (body: unknown, status = 200) => Response.json(body, { status, headers: corsHeaders });
const uuid = (value: unknown): value is string => typeof value === "string" && /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value);
const opaqueId = (value: unknown): value is string => typeof value === "string" && /^[A-Za-z0-9_-]{8,160}$/.test(value);
const mid = (value: unknown): value is string => typeof value === "string" && /^[A-Za-z0-9_-]{1,32}$/.test(value);
const sdp = (value: unknown): value is { type: "offer" | "answer"; sdp: string } => Boolean(value) && typeof value === "object" && ((value as any).type === "offer" || (value as any).type === "answer") && typeof (value as any).sdp === "string" && (value as any).sdp.length < 200000;

type RequestBody = {
  voiceSessionId?: unknown; conversationId?: unknown;
  action?: unknown;
  sessionId?: unknown;
  remoteSessionId?: unknown;
  tracks?: unknown;
  trackNames?: unknown;
  sessionDescription?: unknown;
};

const MAX_TRACKS = 4;

function publishTracks(value: unknown): { mid: string; trackName: string }[] | null {
  if (!Array.isArray(value) || value.length === 0 || value.length > MAX_TRACKS) return null;
  const parsed: { mid: string; trackName: string }[] = [];
  for (const entry of value) {
    if (!entry || typeof entry !== "object") return null;
    const entryMid = (entry as Record<string, unknown>).mid;
    const entryTrackName = (entry as Record<string, unknown>).trackName;
    if (!mid(entryMid) || !opaqueId(entryTrackName)) return null;
    parsed.push({ mid: entryMid, trackName: entryTrackName });
  }
  return parsed;
}

function trackNameList(value: unknown): string[] | null {
  if (!Array.isArray(value) || value.length === 0 || value.length > MAX_TRACKS) return null;
  return value.every(opaqueId) ? (value as string[]) : null;
}

function conciseCloudflareError(payload: unknown) {
  if (!payload || typeof payload !== "object") return undefined;
  const value = payload as Record<string, unknown>;
  const message = typeof value.errorDescription === "string"
    ? value.errorDescription
    : typeof value.error === "string"
      ? value.error
      : typeof value.message === "string"
        ? value.message
        : undefined;
  const code = typeof value.errorCode === "string" || typeof value.errorCode === "number"
    ? String(value.errorCode)
    : undefined;
  return { code, message: message?.slice(0, 240) };
}

const handler = withSupabase({ auth: "user" }, async (req, ctx) => {
  if (req.method !== "POST") return json({ error: "POST required" }, 405);
  const body = await req.json().catch(() => null) as RequestBody | null;
  const userId = ctx.userClaims?.id;
  if (!userId || !uuid(body?.voiceSessionId) || !uuid(body?.conversationId) || typeof body?.action !== "string") return json({ error: "Invalid realtime request" }, 400);

  const admin = ctx.supabaseAdmin as any;
  const [{ data: participant }, { data: conversation }] = await Promise.all([
    admin.from("voice_participants").select("session_id").eq("session_id", body.voiceSessionId).gt("last_seen_at", new Date(Date.now() - 120_000).toISOString()).eq("conversation_id", body.conversationId).eq("user_id", userId).maybeSingle(),
    admin.from("conversations").select("user1_id,user2_id").eq("id", body.conversationId).maybeSingle(),
  ]);
  if (!participant || !conversation || (conversation.user1_id !== userId && conversation.user2_id !== userId)) {
    return json({ error: "Join voice before using screen share." }, 403);
  }

  if (!(await allowRequest(admin, userId, "cloudflare-realtime", 40))) return json({ error: "Too many requests. Try again shortly." }, 429);

  const appId = Deno.env.get("CLOUDFLARE_CALLS_APP_ID");
  const secret = Deno.env.get("CLOUDFLARE_CALLS_APP_SECRET");
  if (!appId || !secret) return json({ error: "Cloudflare screen sharing is not configured." }, 503);

  // PostgREST does not expose private, even to service_role. Narrow invoker
  // RPCs provide server-only access without exposing the schema to clients.
  const readSession = async (sessionId: string) => {
    const { data, error } = await admin.rpc("read_cloudflare_screen_session", {
      p_session_id: sessionId, p_conversation_id: body.conversationId,
    });
    if (error) {
      console.error(JSON.stringify({ event: "cloudflare_screen_session_read_failed", code: error.code }));
      throw new Error("Could not read the screen-share session.");
    }
    return data as { owner_id: string; voice_session_id: string; track_name: string | null } | null;
  };
  const ownedSession = (session: Awaited<ReturnType<typeof readSession>>) => session?.owner_id === userId && session.voice_session_id === body.voiceSessionId;
  const saveSession = (sessionId: string, trackName: string | null = null) => admin.rpc("save_cloudflare_screen_session", {
    p_session_id: sessionId, p_conversation_id: body.conversationId,
    p_owner_id: userId, p_voice_session_id: body.voiceSessionId, p_track_name: trackName,
  });

  let path = "";
  let method = "POST";
  let payload: Record<string, unknown> = {};
  let createdSession = false;
  let publishedTrackNames: string[] | null = null;

  if (body.action === "create" && (body.sessionDescription === undefined || sdp(body.sessionDescription))) {
    path = "/sessions/new";
    // An SDP-less session is intentional: media negotiation happens in
    // tracks/new. Keep accepting a valid offer for older clients.
    payload = body.sessionDescription === undefined ? {} : { sessionDescription: body.sessionDescription };
    createdSession = true;
  } else if (body.action === "publish" && opaqueId(body.sessionId) && sdp(body.sessionDescription) && publishTracks(body.tracks)) {
    const tracks = publishTracks(body.tracks)!;
    const session = await readSession(body.sessionId);
    if (!ownedSession(session)) return json({ error: "That screen-share session is no longer active." }, 403);
    path = `/sessions/${body.sessionId}/tracks/new`;
    payload = { sessionDescription: body.sessionDescription, tracks: tracks.map((track) => ({ location: "local", mid: track.mid, trackName: track.trackName })) };
    // A screen share can publish more than one track (video, and audio when
    // available) - they're stored comma-joined in the same text column
    // subscribe validates against below, rather than adding a new column.
    publishedTrackNames = tracks.map((track) => track.trackName);
  } else if (body.action === "subscribe" && opaqueId(body.sessionId) && opaqueId(body.remoteSessionId) && trackNameList(body.trackNames)) {
    const names = trackNameList(body.trackNames)!;
    const [viewer, remote] = await Promise.all([
      readSession(body.sessionId), readSession(body.remoteSessionId),
    ]);
    const availableNames = new Set(String(remote?.track_name ?? "").split(","));
    if (!ownedSession(viewer) || !remote || remote.owner_id === userId || !names.every((name) => availableNames.has(name))) {

      return json({ error: "The shared screen is no longer available." }, 404);
    }
    const { data: remoteLease } = await admin.from("voice_participants").select("session_id").eq("conversation_id", body.conversationId).eq("user_id", remote?.owner_id).eq("session_id", remote?.voice_session_id).gt("last_seen_at", new Date(Date.now() - 120_000).toISOString()).maybeSingle();
    if (!remoteLease) return json({ error: "The screen publisher has left voice." }, 403);
    path = `/sessions/${body.sessionId}/tracks/new`;
    payload = { tracks: names.map((trackName) => ({ location: "remote", sessionId: body.remoteSessionId, trackName })) };
  } else if (body.action === "renegotiate" && opaqueId(body.sessionId) && sdp(body.sessionDescription)) {
    const session = await readSession(body.sessionId);
    if (!ownedSession(session)) return json({ error: "That viewing session is no longer active." }, 403);
    path = `/sessions/${body.sessionId}/renegotiate`;
    method = "PUT";
    payload = { sessionDescription: body.sessionDescription };
  } else {
    return json({ error: "Invalid screen-share operation" }, 400);
  }

  // Cloudflare distinguishes no body from {}: sending an empty JSON object
  // selects the offer-create schema and fails validation of sessionDescription.
  const hasBody = !(body.action === "create" && body.sessionDescription === undefined);
  const response = await fetch(`https://rtc.live.cloudflare.com/v1/apps/${appId}${path}`, {
    method,
    headers: { Authorization: `Bearer ${secret}`, ...(hasBody ? { "Content-Type": "application/json" } : {}) },
    ...(hasBody ? { body: JSON.stringify(payload) } : {}), signal: AbortSignal.timeout(8_000),
  });
  const result = await response.json().catch(() => null);
  const trackError = result && Array.isArray(result.tracks) ? result.tracks.find((track: Record<string, unknown>) => track.errorCode) : null;
  const upstreamError = conciseCloudflareError(trackError ?? result);
  if (!response.ok || trackError || (result && typeof result === "object" && "errorCode" in result)) {
    console.error(JSON.stringify({ event: "cloudflare_screen_request_failed", action: body.action, status: response.status, code: upstreamError?.code, message: upstreamError?.message }));
    return json({ error: "Cloudflare screen-share request failed.", code: upstreamError?.code ?? `http_${response.status}` }, 502);
  }

  if (createdSession) {
    const sessionId = result && typeof result === "object" && typeof (result as Record<string, unknown>).sessionId === "string"
      ? (result as Record<string, string>).sessionId
      : null;
    if (!sessionId || !opaqueId(sessionId)) {
      console.error(JSON.stringify({ event: "cloudflare_screen_missing_session_id" }));
      return json({ error: "Cloudflare did not return a usable screen-share session." }, 502);
    }
    const { data: saved, error } = await saveSession(sessionId);
    if (error || saved !== true) {
      console.error(JSON.stringify({ event: "cloudflare_screen_session_persist_failed", code: error?.code ?? "lease_expired" }));
      return json({ error: "Could not secure the screen-share session." }, 500);
    }
  }

  if (publishedTrackNames) {
    const { data: saved, error } = await saveSession(body.sessionId as string, publishedTrackNames.join(","));
    if (error || saved !== true) {
      console.error(JSON.stringify({ event: "cloudflare_screen_tracks_persist_failed", code: error?.code ?? "lease_expired" }));
      return json({ error: "Could not register the published screen tracks." }, 500);
    }
  }

  return json(result);
});

export default {
  fetch(req: Request) {
    if (req.method === "OPTIONS") return Promise.resolve(new Response("ok", { headers: corsHeaders }));
    return handler(req);
  },
};
