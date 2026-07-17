import "@supabase/functions-js/edge-runtime.d.ts";
import { withSupabase } from "@supabase/server";
const corsHeaders = { "Access-Control-Allow-Origin": "*", "Access-Control-Allow-Headers": "authorization, apikey, content-type, x-client-info", "Access-Control-Allow-Methods": "POST, OPTIONS" };
const json = (body: unknown, status = 200) => Response.json(body, { status, headers: corsHeaders });
const uuid = (value: unknown): value is string => typeof value === "string" && /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value);
const sdp = (value: unknown): value is { type: "offer" | "answer"; sdp: string } => Boolean(value) && typeof value === "object" && ((value as any).type === "offer" || (value as any).type === "answer") && typeof (value as any).sdp === "string" && (value as any).sdp.length < 200000;
const handler = withSupabase({ auth: "user" }, async (req, ctx) => {
  if (req.method !== "POST") return json({ error: "POST required" }, 405);
  const body = await req.json().catch(() => null) as { conversationId?: unknown; action?: unknown; sessionId?: unknown; remoteSessionId?: unknown; trackName?: unknown; mid?: unknown; sessionDescription?: unknown } | null;
  const userId = ctx.userClaims?.id;
  if (!userId || !uuid(body?.conversationId) || typeof body?.action !== "string") return json({ error: "Invalid realtime request" }, 400);
  const admin = ctx.supabaseAdmin as any;
  const { data: participant } = await admin.from("voice_participants").select("session_id").eq("conversation_id", body.conversationId).eq("user_id", userId).maybeSingle();
  if (!participant) return json({ error: "Join voice before using screen share." }, 403);
  const appId = Deno.env.get("CLOUDFLARE_CALLS_APP_ID"); const secret = Deno.env.get("CLOUDFLARE_CALLS_APP_SECRET");
  if (!appId || !secret) return json({ error: "Cloudflare screen sharing is not configured." }, 503);
  let path = ""; let method = "POST"; let payload: Record<string, unknown> = {};
  if (body.action === "create" && sdp(body.sessionDescription)) { path = "/sessions/new"; payload = { sessionDescription: body.sessionDescription }; }
  else if (body.action === "publish" && typeof body.sessionId === "string" && typeof body.mid === "string" && typeof body.trackName === "string" && sdp(body.sessionDescription)) { path = `/sessions/${body.sessionId}/tracks/new`; payload = { sessionDescription: body.sessionDescription, tracks: [{ location: "local", mid: body.mid, trackName: body.trackName }] }; }
  else if (body.action === "subscribe" && typeof body.sessionId === "string" && typeof body.remoteSessionId === "string" && typeof body.trackName === "string") { path = `/sessions/${body.sessionId}/tracks/new`; payload = { tracks: [{ location: "remote", sessionId: body.remoteSessionId, trackName: body.trackName }] }; }
  else if (body.action === "renegotiate" && typeof body.sessionId === "string" && sdp(body.sessionDescription)) { path = `/sessions/${body.sessionId}/renegotiate`; method = "PUT"; payload = { sessionDescription: body.sessionDescription }; }
  else return json({ error: "Invalid screen-share operation" }, 400);
  const response = await fetch(`https://rtc.live.cloudflare.com/v1/apps/${appId}${path}`, { method, headers: { Authorization: `Bearer ${secret}`, "Content-Type": "application/json" }, body: JSON.stringify(payload) });
  const result = await response.json().catch(() => null);
  if (!response.ok || (result && typeof result === "object" && "errorCode" in result)) return json({ error: "Cloudflare screen-share request failed." }, 502);
  return json(result);
});
export default { fetch(req: Request) { if (req.method === "OPTIONS") return Promise.resolve(new Response("ok", { headers: corsHeaders })); return handler(req); } };