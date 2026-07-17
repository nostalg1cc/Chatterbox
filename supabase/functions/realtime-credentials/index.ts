import "@supabase/functions-js/edge-runtime.d.ts";
import { withSupabase } from "@supabase/server";

const corsHeaders = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Headers": "authorization, apikey, content-type, x-client-info",
  "Access-Control-Allow-Methods": "POST, OPTIONS",
};

function json(body: unknown, status = 200) {
  return Response.json(body, { status, headers: corsHeaders });
}

function isUuid(value: unknown): value is string {
  return typeof value === "string" && /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value);
}

function validTurnUrl(value: unknown): value is string {
  return typeof value === "string" && /^(turn|turns):turn\.cloudflare\.com:/i.test(value) && !value.includes(":53");
}

const handler = withSupabase({ auth: "user" }, async (req, ctx) => {
  if (req.method !== "POST") return json({ error: "POST required" }, 405);
  const body = await req.json().catch(() => null) as { conversationId?: unknown } | null;
  const userId = ctx.userClaims?.id;
  if (!userId || !isUuid(body?.conversationId)) return json({ error: "Invalid voice room" }, 400);

  const admin = ctx.supabaseAdmin as any;
  const [{ data: participant }, { data: conversation }] = await Promise.all([
    admin.from("voice_participants").select("session_id").eq("conversation_id", body.conversationId).eq("user_id", userId).maybeSingle(),
    admin.from("conversations").select("user1_id,user2_id").eq("id", body.conversationId).maybeSingle(),
  ]);
  if (!participant || !conversation || (conversation.user1_id !== userId && conversation.user2_id !== userId)) {
    return json({ error: "Join this voice channel before requesting relay credentials." }, 403);
  }

  const keyId = Deno.env.get("CLOUDFLARE_TURN_KEY_ID");
  const keySecret = Deno.env.get("CLOUDFLARE_TURN_KEY_SECRET");
  if (!keyId || !keySecret) return json({ error: "Cloudflare relay is not configured yet." }, 503);

  const response = await fetch(
    `https://rtc.live.cloudflare.com/v1/turn/keys/${keyId}/credentials/generate`,
    {
      method: "POST",
      headers: { Authorization: `Bearer ${keySecret}`, "Content-Type": "application/json" },
      body: JSON.stringify({ ttl: 43_200 }),
    }
  );
  if (!response.ok) {
    console.error("Cloudflare TURN credential request failed", response.status);
    return json({ error: "Cloudflare relay credentials could not be issued." }, 503);
  }

  const payload = await response.json() as { iceServers?: { urls?: unknown; username?: unknown; credential?: unknown } };
  const urls = Array.isArray(payload.iceServers?.urls) ? payload.iceServers.urls.filter(validTurnUrl) : [];
  if (!urls.length || typeof payload.iceServers?.username !== "string" || typeof payload.iceServers?.credential !== "string") {
    return json({ error: "Cloudflare returned invalid relay credentials." }, 503);
  }

  return json({
    iceServers: [
      { urls: ["stun:stun.cloudflare.com:3478"] },
      { urls, username: payload.iceServers.username, credential: payload.iceServers.credential, credentialType: "password" },
    ],
    expiresAt: Date.now() + 43_200_000,
  });
});

export default {
  fetch(req: Request) {
    if (req.method === "OPTIONS") return Promise.resolve(new Response("ok", { headers: corsHeaders }));
    return handler(req);
  },
};