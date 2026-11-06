// Guarded Cloudflare screen sharing for Nitro Hero.
//
// Every share and every view holds a lease from the database, which refuses it once the
// month's budget is spent. Leases run out in 10 minutes, or 45 seconds without a renewal,
// and then a scheduled call here force-closes their tracks at Cloudflare. That last step
// matters: a client that stops renewing can otherwise keep receiving media.
//
// cloudflare-realtime stays as it is for installed main Nitro clients.
import "@supabase/functions-js/edge-runtime.d.ts";
import { withSupabase } from "@supabase/server";
import { createClient } from "npm:@supabase/supabase-js@2.110.2";
import { allowRequest } from "../_shared/quota.ts";

const corsHeaders = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Headers": "authorization, apikey, content-type, x-client-info, x-cleanup-key",
  "Access-Control-Allow-Methods": "POST, OPTIONS",
};
const json = (body: unknown, status = 200) => Response.json(body, { status, headers: corsHeaders });
const uuid = (value: unknown): value is string => typeof value === "string" && /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value);
const opaqueId = (value: unknown): value is string => typeof value === "string" && /^[A-Za-z0-9_-]{8,160}$/.test(value);
const mid = (value: unknown): value is string => typeof value === "string" && /^[A-Za-z0-9_-]{1,32}$/.test(value);
const sdp = (value: unknown, type: "offer" | "answer"): value is { type: string; sdp: string } =>
  Boolean(value) && typeof value === "object" && (value as any).type === type && typeof (value as any).sdp === "string" && (value as any).sdp.length < 200_000;

const REFUSALS: Record<string, [string, number]> = {
  budget: ["This month's screen sharing allowance has been used up.", 429],
  cleanup_pending: ["Screen sharing is paused until an earlier share finishes closing.", 503],
  already_sharing: ["You're already sharing your screen.", 409],
  one_viewer: ["Someone is already watching this screen.", 409],
  not_available: ["That screen share has ended.", 404],
};

async function cloudflare(path: string, method: string, payload?: unknown) {
  const appId = Deno.env.get("CLOUDFLARE_CALLS_APP_ID");
  const secret = Deno.env.get("CLOUDFLARE_CALLS_APP_SECRET");
  if (!appId || !secret) throw new Error("not_configured");
  const response = await fetch(`https://rtc.live.cloudflare.com/v1/apps/${appId}${path}`, {
    method,
    headers: { Authorization: `Bearer ${secret}`, ...(payload === undefined ? {} : { "Content-Type": "application/json" }) },
    ...(payload === undefined ? {} : { body: JSON.stringify(payload) }),
    signal: AbortSignal.timeout(8_000),
  });
  const result = await response.json().catch(() => null) as Record<string, any> | null;
  const trackError = Array.isArray(result?.tracks) ? result.tracks.find((track: Record<string, unknown>) => track.errorCode) : null;
  if (!response.ok || trackError || (result && "errorCode" in result)) {
    console.error(JSON.stringify({ event: "screen_share_provider_failed", path: path.split("/").pop(), status: response.status, code: trackError?.errorCode ?? result?.errorCode }));
    throw new Error("provider_failed");
  }
  return result ?? {};
}

// A track that's absent or already closed counts as closed. Anything else, or a track the
// reply doesn't mention, is kept for a retry, because media may still be flowing.
async function closeTracks(sessionId: string | null, mids: string[]): Promise<boolean> {
  if (!sessionId || mids.length === 0) return true;
  const appId = Deno.env.get("CLOUDFLARE_CALLS_APP_ID");
  const secret = Deno.env.get("CLOUDFLARE_CALLS_APP_SECRET");
  if (!appId || !secret) return false;
  const response = await fetch(`https://rtc.live.cloudflare.com/v1/apps/${appId}/sessions/${sessionId}/tracks/close`, {
    method: "PUT",
    headers: { Authorization: `Bearer ${secret}`, "Content-Type": "application/json" },
    body: JSON.stringify({ tracks: mids.map((value) => ({ mid: value })), force: true }),
    signal: AbortSignal.timeout(8_000),
  });
  const result = await response.json().catch(() => null) as { errorCode?: string; tracks?: { mid: string; errorCode?: string }[] } | null;
  // 410 means the session itself has ended, usually because its peer closed first, so
  // nothing can still be forwarding.
  if (response.status === 410) return true;
  const closed = response.ok && mids.every((value) => {
    const track = result?.tracks?.find((entry) => entry.mid === value);
    return track !== undefined && (!track.errorCode || track.errorCode === "close_track_error");
  });
  if (!closed) console.error(JSON.stringify({ event: "screen_share_close_failed", status: response.status, code: result?.errorCode, tracks: result?.tracks?.map((track) => track.errorCode) }));
  return closed;
}

async function finish(admin: any, lease: { id: string; session_id: string | null; mids: string[] | null }) {
  const closed = await closeTracks(lease.session_id, lease.mids ?? []).catch(() => false);
  if (closed) {
    const { error } = await admin.rpc("screen_share_finish", { p_lease_id: lease.id });
    return !error;
  }
  await admin.rpc("screen_share_note_failure", { p_lease_id: lease.id });
  return false;
}

async function reap(admin: any) {
  const { data, error } = await admin.rpc("screen_share_due", { p_limit: 50 });
  if (error) return json({ error: "Could not list due shares." }, 500);
  const results = await Promise.all((data ?? []).map((lease: any) => finish(admin, lease)));
  const failed = results.filter((closed) => !closed).length;
  if (failed) console.error(JSON.stringify({ event: "screen_share_reap_failed", failed }));
  return json({ closed: results.length - failed, failed });
}

const handler = withSupabase({ auth: "user" }, async (req, ctx) => {
  if (req.method !== "POST") return json({ error: "POST required" }, 405);
  const body = await req.json().catch(() => null) as Record<string, any> | null;
  const userId = ctx.userClaims?.id;
  if (!userId || !uuid(body?.voiceSessionId) || !uuid(body?.conversationId) || typeof body?.action !== "string") {
    return json({ error: "Invalid screen-share request." }, 400);
  }
  const admin = ctx.supabaseAdmin as any;

  // Renew and close must keep working however full the request budget is, so they're
  // checked by lease owner alone. Stopping media is never refused.
  if (body.action === "renew" || body.action === "close") {
    if (!uuid(body.leaseId)) return json({ error: "Invalid lease." }, 400);
    if (body.action === "renew") {
      const { data, error } = await admin.rpc("screen_share_renew", { p_lease_id: body.leaseId, p_user_id: userId });
      // The client treats an error as a missed renewal, and a false as the end.
      if (error) return json({ error: "Could not renew." }, 500);
      return json({ ok: data === true });
    }
    const { data: lease, error } = await admin.rpc("screen_share_owned", { p_lease_id: body.leaseId, p_user_id: userId });
    if (error) return json({ error: "Could not close." }, 500);
    if (lease) await finish(admin, lease); // A failed close stays due, so the scheduled call retries it.
    return json({ ok: true });
  }

  const [{ data: participant }, { data: conversation }] = await Promise.all([
    admin.from("voice_participants").select("session_id").eq("session_id", body.voiceSessionId).gt("last_seen_at", new Date(Date.now() - 120_000).toISOString()).eq("conversation_id", body.conversationId).eq("user_id", userId).maybeSingle(),
    admin.from("conversations").select("user1_id,user2_id").eq("id", body.conversationId).maybeSingle(),
  ]);
  if (!participant || !conversation || (conversation.user1_id !== userId && conversation.user2_id !== userId)) {
    return json({ error: "Join voice before using screen share." }, 403);
  }
  if (!(await allowRequest(admin, userId, "screen-share", 30))) return json({ error: "Too many requests. Try again shortly." }, 429);

  const scope = { p_conversation_id: body.conversationId, p_user_id: userId, p_voice_session_id: body.voiceSessionId };
  const saveSession = (sessionId: string, trackNames: string | null = null) => admin.rpc("save_cloudflare_screen_session", {
    p_session_id: sessionId, p_conversation_id: body.conversationId,
    p_owner_id: userId, p_voice_session_id: body.voiceSessionId, p_track_name: trackNames,
  });
  const admit = async (kind: "publish" | "view", remoteSessionId: string | null) => {
    const { data, error } = await admin.rpc("screen_share_admit", { ...scope, p_kind: kind, p_remote_session_id: remoteSessionId });
    if (error) {
      console.error(JSON.stringify({ event: "screen_share_admit_failed", code: error.code }));
      return { refusal: json({ error: "Screen sharing isn't available right now." }, 503) };
    }
    if (!data?.ok) {
      const [message, status] = REFUSALS[data?.reason] ?? ["Screen sharing isn't available right now.", 503];
      return { refusal: json({ error: message, code: data?.reason }, status) };
    }
    return { lease: data as { id: string; seconds_left: number } };
  };
  const owned = async (leaseId: unknown, sessionId: unknown) => {
    if (!uuid(leaseId) || !opaqueId(sessionId)) return null;
    const { data } = await admin.rpc("screen_share_owned", { p_lease_id: leaseId, p_user_id: userId });
    return data?.session_id === sessionId ? data : null;
  };

  try {
    if (body.action === "publish-start") {
      const { lease, refusal } = await admit("publish", null);
      if (refusal) return refusal;
      // From here on, any failure gives the lease back, so a retry isn't refused as busy.
      try {
        const created = await cloudflare("/sessions/new", "POST");
        if (!opaqueId(created.sessionId)) throw new Error("provider_failed");
        const { data: saved } = await saveSession(created.sessionId);
        if (saved !== true) throw new Error("session_not_saved");
        await admin.rpc("screen_share_attach", { p_lease_id: lease!.id, p_session_id: created.sessionId, p_mids: [] });
        return json({ leaseId: lease!.id, sessionId: created.sessionId, secondsLeft: lease!.seconds_left });
      } catch (error) {
        await admin.rpc("screen_share_finish", { p_lease_id: lease!.id });
        throw error;
      }
    }

    if (body.action === "publish") {
      if (!mid(body.mid) || !opaqueId(body.trackName) || !sdp(body.sessionDescription, "offer")) return json({ error: "Invalid share." }, 400);
      const lease = await owned(body.leaseId, body.sessionId);
      if (!lease) return json({ error: "That screen share has ended." }, 404);
      const reply = await cloudflare(`/sessions/${body.sessionId}/tracks/new`, "POST", {
        sessionDescription: body.sessionDescription,
        tracks: [{ location: "local", mid: body.mid, trackName: body.trackName }],
      });
      await admin.rpc("screen_share_attach", { p_lease_id: body.leaseId, p_session_id: body.sessionId, p_mids: [body.mid] });
      const { data: saved } = await saveSession(body.sessionId, body.trackName);
      if (saved !== true) return json({ error: "Could not register the shared screen." }, 500);
      return json({ sessionDescription: reply.sessionDescription });
    }

    if (body.action === "view-start") {
      const names = Array.isArray(body.trackNames) && body.trackNames.length > 0 && body.trackNames.length <= 4 && body.trackNames.every(opaqueId) ? body.trackNames as string[] : null;
      if (!names || !opaqueId(body.remoteSessionId)) return json({ error: "Invalid screen." }, 400);
      const { data: remote } = await admin.rpc("read_cloudflare_screen_session", { p_session_id: body.remoteSessionId, p_conversation_id: body.conversationId });
      const available = new Set(String(remote?.track_name ?? "").split(","));
      if (!remote || remote.owner_id === userId || !names.every((name) => available.has(name))) return json({ error: "That screen share has ended." }, 404);
      const { data: publisher } = await admin.from("voice_participants").select("session_id").eq("conversation_id", body.conversationId).eq("user_id", remote.owner_id).eq("session_id", remote.voice_session_id).gt("last_seen_at", new Date(Date.now() - 120_000).toISOString()).maybeSingle();
      if (!publisher) return json({ error: "They've left the call." }, 404);

      // The full allowance is reserved before Cloudflare is asked for anything.
      const { lease, refusal } = await admit("view", body.remoteSessionId);
      if (refusal) return refusal;
      let sessionId: string | null = null;
      try {
        const created = await cloudflare("/sessions/new", "POST");
        if (!opaqueId(created.sessionId)) throw new Error("provider_failed");
        sessionId = created.sessionId;
        const { data: saved } = await saveSession(created.sessionId);
        if (saved !== true) throw new Error("session_not_saved");
        const reply = await cloudflare(`/sessions/${created.sessionId}/tracks/new`, "POST", {
          tracks: names.map((trackName) => ({ location: "remote", sessionId: body.remoteSessionId, trackName })),
        });
        const mids = (reply.tracks ?? []).map((track: { mid?: string }) => track.mid).filter(mid);
        await admin.rpc("screen_share_attach", { p_lease_id: lease!.id, p_session_id: created.sessionId, p_mids: mids });
        if (reply.sessionDescription?.type !== "offer") throw new Error("provider_failed");
        return json({ leaseId: lease!.id, sessionId: created.sessionId, secondsLeft: lease!.seconds_left, sessionDescription: reply.sessionDescription });
      } catch (error) {
        // With mids attached it stays open and due, so the reservation isn't refunded
        // until Cloudflare has confirmed those tracks closed.
        if (!sessionId) await admin.rpc("screen_share_finish", { p_lease_id: lease!.id });
        else await admin.rpc("screen_share_expire", { p_lease_id: lease!.id });
        throw error;
      }
    }

    if (body.action === "renegotiate") {
      if (!sdp(body.sessionDescription, "answer")) return json({ error: "Invalid answer." }, 400);
      if (!(await owned(body.leaseId, body.sessionId))) return json({ error: "That screen share has ended." }, 404);
      await cloudflare(`/sessions/${body.sessionId}/renegotiate`, "PUT", { sessionDescription: body.sessionDescription });
      return json({ ok: true });
    }
  } catch (error) {
    const reason = error instanceof Error ? error.message : "unknown";
    if (reason === "not_configured") return json({ error: "Screen sharing isn't configured." }, 503);
    console.error(JSON.stringify({ event: "screen_share_failed", action: body.action, reason }));
    return json({ error: "Screen sharing could not connect. Please try again." }, 502);
  }
  return json({ error: "Invalid screen-share operation." }, 400);
});

export default {
  async fetch(req: Request) {
    if (req.method === "OPTIONS") return new Response("ok", { headers: corsHeaders });
    // The scheduled job has no user. It proves itself with the shared cleanup key.
    const cleanupKey = req.headers.get("x-cleanup-key");
    if (cleanupKey !== null) {
      if (cleanupKey.length !== 64) return json({ error: "Forbidden" }, 403);
      const admin = createClient(Deno.env.get("SUPABASE_URL")!, Deno.env.get("SUPABASE_SERVICE_ROLE_KEY")!, { auth: { persistSession: false } });
      const { data: authorized, error } = await admin.rpc("validate_media_cleanup_key", { p_key: cleanupKey });
      if (error || authorized !== true) return json({ error: "Forbidden" }, 403);
      return reap(admin);
    }
    return handler(req);
  },
};
