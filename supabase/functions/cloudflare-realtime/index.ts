import "@supabase/functions-js/edge-runtime.d.ts";
import { withSupabase } from "@supabase/server";

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
  conversationId?: unknown;
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
  if (!userId || !uuid(body?.conversationId) || typeof body?.action !== "string") return json({ error: "Invalid realtime request" }, 400);

  const admin = ctx.supabaseAdmin as any;
  const [{ data: participant }, { data: conversation }] = await Promise.all([
    admin.from("voice_participants").select("session_id").eq("conversation_id", body.conversationId).eq("user_id", userId).maybeSingle(),
    admin.from("conversations").select("user1_id,user2_id").eq("id", body.conversationId).maybeSingle(),
  ]);
  if (!participant || !conversation || (conversation.user1_id !== userId && conversation.user2_id !== userId)) {
    return json({ error: "Join voice before using screen share." }, 403);
  }

  const appId = Deno.env.get("CLOUDFLARE_CALLS_APP_ID");
  const secret = Deno.env.get("CLOUDFLARE_CALLS_APP_SECRET");
  if (!appId || !secret) return json({ error: "Cloudflare screen sharing is not configured." }, 503);

  const sessions = admin.schema("private").from("cloudflare_screen_sessions");
  await sessions.delete().lte("expires_at", new Date().toISOString());

  let path = "";
  let method = "POST";
  let payload: Record<string, unknown> = {};
  let createdSession = false;
  let publishedTrackNames: string[] | null = null;

  if (body.action === "create" && sdp(body.sessionDescription)) {
    path = "/sessions/new";
    payload = { sessionDescription: body.sessionDescription };
    createdSession = true;
  } else if (body.action === "publish" && opaqueId(body.sessionId) && sdp(body.sessionDescription) && publishTracks(body.tracks)) {
    const tracks = publishTracks(body.tracks)!;
    const { data: session } = await sessions.select("session_id").eq("session_id", body.sessionId).eq("conversation_id", body.conversationId).eq("owner_id", userId).gt("expires_at", new Date().toISOString()).maybeSingle();
    if (!session) return json({ error: "That screen-share session is no longer active." }, 403);
    path = `/sessions/${body.sessionId}/tracks/new`;
    payload = { sessionDescription: body.sessionDescription, tracks: tracks.map((track) => ({ location: "local", mid: track.mid, trackName: track.trackName })) };
    // A screen share can publish more than one track (video, and audio when
    // available) - they're stored comma-joined in the same text column
    // subscribe validates against below, rather than adding a new column.
    publishedTrackNames = tracks.map((track) => track.trackName);
  } else if (body.action === "subscribe" && opaqueId(body.sessionId) && opaqueId(body.remoteSessionId) && trackNameList(body.trackNames)) {
    const names = trackNameList(body.trackNames)!;
    const [{ data: viewer }, { data: remote }] = await Promise.all([
      sessions.select("session_id").eq("session_id", body.sessionId).eq("conversation_id", body.conversationId).eq("owner_id", userId).gt("expires_at", new Date().toISOString()).maybeSingle(),
      sessions.select("track_name").eq("session_id", body.remoteSessionId).eq("conversation_id", body.conversationId).neq("owner_id", userId).gt("expires_at", new Date().toISOString()).maybeSingle(),
    ]);
    const availableNames = new Set(String(remote?.track_name ?? "").split(","));
    if (!viewer || !remote || !names.every((name) => availableNames.has(name))) {
      return json({ error: "The shared screen is no longer available." }, 404);
    }
    path = `/sessions/${body.sessionId}/tracks/new`;
    payload = { tracks: names.map((trackName) => ({ location: "remote", sessionId: body.remoteSessionId, trackName })) };
  } else if (body.action === "renegotiate" && opaqueId(body.sessionId) && sdp(body.sessionDescription)) {
    const { data: session } = await sessions.select("session_id").eq("session_id", body.sessionId).eq("conversation_id", body.conversationId).eq("owner_id", userId).gt("expires_at", new Date().toISOString()).maybeSingle();
    if (!session) return json({ error: "That viewing session is no longer active." }, 403);
    path = `/sessions/${body.sessionId}/renegotiate`;
    method = "PUT";
    payload = { sessionDescription: body.sessionDescription };
  } else {
    return json({ error: "Invalid screen-share operation" }, 400);
  }

  const response = await fetch(`https://rtc.live.cloudflare.com/v1/apps/${appId}${path}`, {
    method,
    headers: { Authorization: `Bearer ${secret}`, "Content-Type": "application/json" },
    body: JSON.stringify(payload),
  });
  const result = await response.json().catch(() => null);
  const upstreamError = conciseCloudflareError(result);
  if (!response.ok || (result && typeof result === "object" && "errorCode" in result)) {
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
    const { error } = await sessions.upsert({
      session_id: sessionId,
      conversation_id: body.conversationId,
      owner_id: userId,
      expires_at: new Date(Date.now() + 2 * 60 * 60 * 1000).toISOString(),
      updated_at: new Date().toISOString(),
    });
    if (error) {
      console.error(JSON.stringify({ event: "cloudflare_screen_session_persist_failed", code: error.code }));
      return json({ error: "Could not secure the screen-share session." }, 500);
    }
  }

  if (publishedTrackNames) {
    await sessions.update({
      track_name: publishedTrackNames.join(","),
      updated_at: new Date().toISOString(),
      expires_at: new Date(Date.now() + 2 * 60 * 60 * 1000).toISOString(),
    }).eq("session_id", body.sessionId);
  }

  return json(result);
});

export default {
  fetch(req: Request) {
    if (req.method === "OPTIONS") return Promise.resolve(new Response("ok", { headers: corsHeaders }));
    return handler(req);
  },
};