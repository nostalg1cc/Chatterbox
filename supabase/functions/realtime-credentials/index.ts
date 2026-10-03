import "@supabase/functions-js/edge-runtime.d.ts";
import { withSupabase } from "@supabase/server";
import { allowRequest } from "../_shared/quota.ts";

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
  if (typeof value !== "string") return false;
  const match = value.match(/^turns?:turn\.cloudflare\.com:(\d+)(?:[/?]|$)/i);
  // Filter the browser-blocked alternate port 53 exactly. A substring check
  // also removes valid TLS port 5349, which can be essential on locked-down
  // networks.
  return Boolean(match && match[1] !== "53");
}

const handler = withSupabase({ auth: "user" }, async (req, ctx) => {
  if (req.method !== "POST") return json({ error: "POST required" }, 405);
  const body = await req.json().catch(() => null) as { voiceSessionId?: unknown; conversationId?: unknown } | null;
  const userId = ctx.userClaims?.id;
  if (!userId || !isUuid(body?.voiceSessionId) || !isUuid(body?.conversationId)) return json({ error: "Invalid voice room" }, 400);

  const admin = ctx.supabaseAdmin as any;
  const [{ data: participant }, { data: conversation }] = await Promise.all([
    admin.from("voice_participants").select("session_id").eq("session_id", body.voiceSessionId).gt("last_seen_at", new Date(Date.now() - 120_000).toISOString()).eq("conversation_id", body.conversationId).eq("user_id", userId).maybeSingle(),
    admin.from("conversations").select("user1_id,user2_id").eq("id", body.conversationId).maybeSingle(),
  ]);
  if (!participant || !conversation || (conversation.user1_id !== userId && conversation.user2_id !== userId)) {
    return json({ error: "Join this voice channel before requesting relay credentials." }, 403);
  }

  if (!(await allowRequest(admin, userId, "realtime-credentials", 8))) return json({ error: "Too many requests. Try again shortly." }, 429);

  const keyId = Deno.env.get("CLOUDFLARE_TURN_KEY_ID");
  const keySecret = Deno.env.get("CLOUDFLARE_TURN_KEY_SECRET");
  if (!keyId || !keySecret) return json({ error: "Cloudflare relay is not configured yet." }, 503);

  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 4_000);
  let response: Response;
  let payload: { iceServers?: { urls?: unknown; username?: unknown; credential?: unknown } | Array<{ urls?: unknown; username?: unknown; credential?: unknown }> } | null;
  try {
    response = await fetch(
      `https://rtc.live.cloudflare.com/v1/turn/keys/${keyId}/credentials/generate-ice-servers`,
      {
        method: "POST",
        headers: { Authorization: `Bearer ${keySecret}`, "Content-Type": "application/json" },
        body: JSON.stringify({ ttl: 43_200 }),
        signal: controller.signal,
      }
    );
    if (!response.ok) {
      console.error("Cloudflare TURN credential request failed", response.status);
      return json({ error: "Cloudflare relay credentials could not be issued." }, 503);
    }
    payload = await response.json().catch(() => null);
  } catch (error) {
    console.error("Cloudflare TURN credential request could not reach the provider", error);
    return json({ error: "Cloudflare relay credentials could not be issued." }, 503);
  } finally {
    clearTimeout(timeout);
  }

  const servers = Array.isArray(payload?.iceServers) ? payload.iceServers : payload?.iceServers ? [payload.iceServers] : [];
  const relays = servers.flatMap((entry) => {
    if (!entry || typeof entry !== "object") return [];
    const urls = (Array.isArray(entry.urls) ? entry.urls : typeof entry.urls === "string" ? [entry.urls] : [])
      .filter(validTurnUrl);
    if (!urls.length || typeof entry.username !== "string" || typeof entry.credential !== "string") return [];
    return [{ urls, username: entry.username, credential: entry.credential, credentialType: "password" as const }];
  });
  if (!relays.length) {
    return json({ error: "Cloudflare returned invalid relay credentials." }, 503);
  }

  return json({
    iceServers: [
      { urls: ["stun:stun.cloudflare.com:3478"] },
      ...relays,
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
