import "@supabase/functions-js/edge-runtime.d.ts";
import { withSupabase } from "@supabase/server";

const BUCKET = "soundboard";
const MAX_BYTES = 512 * 1024;
const MAX_DURATION_MS = 15_000;
const PLAYBACK_SYNC_LEAD_MS = 200;
const corsHeaders = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Headers": "authorization, apikey, content-type, x-client-info",
  "Access-Control-Allow-Methods": "POST, OPTIONS",
};

type RequestBody = {
  mode?: "list" | "reserve" | "finalize" | "discard" | "rename" | "delete" | "play" | "preview";
  soundId?: string;
  name?: string;
  sizeBytes?: number;
  durationMs?: number;
  conversationId?: string;
};

function json(body: unknown, status = 200) {
  return Response.json(body, { status, headers: corsHeaders });
}

function isUuid(value: unknown): value is string {
  return typeof value === "string" && /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value);
}

async function activeVoiceMembers(admin: any, conversationId: string, userId: string): Promise<string[] | null> {
  const [{ data: conversation }, { data: participant }] = await Promise.all([
    admin.from("conversations").select("user1_id,user2_id").eq("id", conversationId).maybeSingle(),
    admin.from("voice_participants").select("session_id").eq("conversation_id", conversationId).eq("user_id", userId).maybeSingle(),
  ]);
  if (!conversation || !participant || (conversation.user1_id !== userId && conversation.user2_id !== userId)) return null;
  return [conversation.user1_id, conversation.user2_id];
}

const handler = withSupabase({ auth: "user" }, async (req, ctx) => {
  if (req.method !== "POST") return json({ error: "POST required" }, 405);
  const userId = ctx.userClaims?.id;
  if (!userId) return json({ error: "A signed-in user is required" }, 401);
  const body = await req.json().catch(() => null) as RequestBody | null;
  if (!body?.mode) return json({ error: "Invalid request" }, 400);
  const admin = ctx.supabaseAdmin as any;

  if (body.mode === "list") {
    if (body.conversationId !== undefined) {
      if (!isUuid(body.conversationId)) return json({ error: "Invalid voice room" }, 400);
      const memberIds = await activeVoiceMembers(admin, body.conversationId, userId);
      if (!memberIds) return json({ error: "Join voice to open the shared soundboard." }, 403);
      const { data, error } = await admin.from("soundboard_sounds").select("id,owner_id,name,storage_path,size_bytes,duration_ms,created_at").in("owner_id", memberIds).order("created_at", { ascending: false });
      if (error) return json({ error: error.message }, 500);
      const sounds = data ?? [];
      const { data: signedUrls, error: signedUrlError } = await admin.storage.from(BUCKET).createSignedUrls(sounds.map((sound) => sound.storage_path), 60);
      if (signedUrlError) return json({ error: signedUrlError.message }, 500);
      const idByPath = new Map(sounds.map((sound) => [sound.storage_path, sound.id]));
      const preloadUrls: Record<string, string> = {};
      for (const entry of signedUrls ?? []) {
        const soundId = entry.path ? idByPath.get(entry.path) : undefined;
        if (soundId && entry.signedUrl) preloadUrls[soundId] = entry.signedUrl;
      }
      return json({ sounds: sounds.map(({ storage_path: _storagePath, ...sound }) => sound), preloadUrls });
    }
    const { data, error } = await ctx.supabase.from("soundboard_sounds").select("*").order("created_at", { ascending: false });
    return error ? json({ error: error.message }, 500) : json({ sounds: data ?? [] });
  }

  if (!isUuid(body.soundId)) return json({ error: "Invalid sound id" }, 400);
  const path = userId + "/" + body.soundId + ".webm";

  if (body.mode === "reserve") {
    const name = body.name?.trim().slice(0, 32);
    const size = Math.floor(Number(body.sizeBytes));
    const duration = Math.floor(Number(body.durationMs));
    if (!name || !Number.isFinite(size) || size < 1 || size > MAX_BYTES || !Number.isFinite(duration) || duration < 100 || duration > MAX_DURATION_MS) {
      return json({ error: "Sound must be 15 seconds or shorter and 512 KiB or smaller." }, 400);
    }
    const { data: reserved, error: reserveError } = await admin.rpc("reserve_soundboard_upload", { p_sound_id: body.soundId, p_owner_id: userId, p_path: path, p_size_bytes: size });
    if (reserveError) return json({ error: reserveError.message }, 500);
    if (!reserved) return json({ error: "Soundboard storage is full (16 MiB per user, 96 MiB shared)." }, 409);
    const { data, error } = await admin.storage.from(BUCKET).createSignedUploadUrl(path);
    if (error || !data?.token) {
      await admin.rpc("release_soundboard_reservation", { p_sound_id: body.soundId });
      return json({ error: error?.message ?? "Unable to create upload URL" }, 500);
    }
    return json({ path, token: data.token, maxFileBytes: MAX_BYTES });
  }

  if (body.mode === "discard") {
    await admin.storage.from(BUCKET).remove([path]);
    await admin.rpc("release_soundboard_reservation", { p_sound_id: body.soundId });
    return json({ discarded: true });
  }

  if (body.mode === "play") {
    if (!isUuid(body.conversationId)) return json({ error: "Invalid voice room" }, 400);
    const memberIds = await activeVoiceMembers(admin, body.conversationId, userId);
    if (!memberIds) return json({ error: "Join voice before using the soundboard." }, 403);
    const { data: sound, error: soundError } = await admin.from("soundboard_sounds").select("*").eq("id", body.soundId).in("owner_id", memberIds).maybeSingle();
    if (!sound || soundError) return json({ error: "Sound is not available in this voice room." }, 404);
    const { data, error } = await admin.storage.from(BUCKET).createSignedUrl(sound.storage_path, 60);
    if (error || !data?.signedUrl) return json({ error: error?.message ?? "Sound could not be opened" }, 500);
    return json({ sound: { id: sound.id, owner_id: sound.owner_id, name: sound.name, duration_ms: sound.duration_ms }, signedUrl: data.signedUrl, playAt: Date.now() + PLAYBACK_SYNC_LEAD_MS });
  }

  const { data: sound, error: soundError } = await admin.from("soundboard_sounds").select("*").eq("id", body.soundId).eq("owner_id", userId).maybeSingle();

  if (body.mode === "preview") {
    if (!sound || soundError) return json({ error: "Sound not found" }, 404);
    const { data, error } = await admin.storage.from(BUCKET).createSignedUrl(sound.storage_path, 60);
    return error || !data?.signedUrl ? json({ error: error?.message ?? "Sound could not be opened" }, 500) : json({ signedUrl: data.signedUrl });
  }

  if (body.mode === "finalize") {
    const name = body.name?.trim().slice(0, 32);
    const size = Math.floor(Number(body.sizeBytes));
    const duration = Math.floor(Number(body.durationMs));
    if (!name || size < 1 || size > MAX_BYTES || duration < 100 || duration > MAX_DURATION_MS) return json({ error: "Invalid sound metadata" }, 400);
    const [{ data: objects, error: listError }, { data: reservation }] = await Promise.all([
      admin.storage.from(BUCKET).list(userId, { search: body.soundId + ".webm", limit: 2 }),
      admin.rpc("soundboard_reservation_bytes", { p_sound_id: body.soundId, p_owner_id: userId }),
    ]);
    const object = objects?.find((entry: any) => entry.name === body.soundId + ".webm");
    const actualSize = Number(object?.metadata?.size);
    if (listError || !object || !reservation || !Number.isFinite(actualSize) || actualSize < 1 || actualSize > Number(reservation) || actualSize > MAX_BYTES) {
      await admin.storage.from(BUCKET).remove([path]);
      await admin.rpc("release_soundboard_reservation", { p_sound_id: body.soundId });
      return json({ error: "Uploaded sound failed verification." }, 400);
    }
    const { data, error } = await admin.from("soundboard_sounds").upsert({ id: body.soundId, owner_id: userId, name, storage_path: path, size_bytes: actualSize, duration_ms: duration }).select().single();
    await admin.rpc("release_soundboard_reservation", { p_sound_id: body.soundId });
    return error ? json({ error: error.message }, 500) : json({ sound: data });
  }

  if (!sound || soundError) return json({ error: "Sound not found" }, 404);

  if (body.mode === "rename") {
    const name = body.name?.trim().slice(0, 32);
    if (!name) return json({ error: "Give the sound a name." }, 400);
    const { data, error } = await admin.from("soundboard_sounds").update({ name }).eq("id", sound.id).eq("owner_id", userId).select().single();
    return error ? json({ error: error.message }, 500) : json({ sound: data });
  }

  if (body.mode === "delete") {
    const { error } = await admin.storage.from(BUCKET).remove([sound.storage_path]);
    if (error) return json({ error: error.message }, 500);
    await admin.from("soundboard_sounds").delete().eq("id", sound.id).eq("owner_id", userId);
    return json({ deleted: true });
  }
  return json({ error: "Unknown mode" }, 400);
});

export default {
  fetch(req: Request) {
    if (req.method === "OPTIONS") return Promise.resolve(new Response("ok", { headers: corsHeaders }));
    return handler(req);
  },
};
