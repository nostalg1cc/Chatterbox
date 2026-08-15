import "@supabase/functions-js/edge-runtime.d.ts";
import { withSupabase } from "@supabase/server";
import { createClient } from "npm:@supabase/supabase-js@2.110.2";

const CHAT_BUCKET = "chat-media";
const CHAT_MEDIA_BUDGET_BYTES = 512 * 1024 * 1024;
// Cloudinary chat media isn't bound by the tiny legacy Supabase Storage
// bucket - it gets its own, much larger budget (the account's free-tier
// storage headroom is currently in the hundreds of MB used out of ~25GB
// available). This is just a runaway-cost safety net, not the thing that
// keeps media around day to day - see media_expires_at handling below.
const CLOUDINARY_CHAT_MEDIA_BUDGET_BYTES = 4 * 1024 * 1024 * 1024;
const LEGACY_MAX_UPLOAD_BYTES = 50 * 1024 * 1024;
const CLOUDINARY_IMAGE_MAX_BYTES = 10 * 1024 * 1024;
const CLOUDINARY_VIDEO_MAX_BYTES = 100 * 1024 * 1024;
const RETENTION_MS = 3 * 24 * 60 * 60 * 1000;
const LIST_PAGE_SIZE = 1000;
const REMOVE_BATCH_SIZE = 100;
const CLOUDINARY_CLOUD_NAME = Deno.env.get("CLOUDINARY_CLOUD_NAME") ?? "lnkoms9m";

const corsHeaders = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Headers": "authorization, apikey, content-type, x-client-info",
  "Access-Control-Allow-Methods": "POST, OPTIONS",
};

type StorageObject = { name: string; created_at?: string | null; metadata?: { size?: number | string } | null };
type MediaKind = "image" | "video";
type Provider = "storage" | "cloudinary";
type RequestBody = {
  mode?: "scheduled" | "capability" | "reserve" | "discard" | "finalize";
  provider?: Provider;
  conversationId?: string;
  messageId?: string;
  path?: string;
  kind?: MediaKind;
  mimeType?: "image/webp" | "video/webm" | "video/mp4";
  uploadMimeType?: string;
  sizeBytes?: number;
};

type CloudMessage = {
  id: string;
  media_path: string;
  media_kind: MediaKind;
  media_size_bytes: number;
  media_expires_at: string | null;
  created_at: string;
};

function json(body: unknown, status = 200) { return Response.json(body, { status, headers: corsHeaders }); }
function uuid(value: unknown): value is string { return typeof value === "string" && /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value); }
function cloudinaryReady() { return Boolean(Deno.env.get("CLOUDINARY_API_KEY") && Deno.env.get("CLOUDINARY_API_SECRET")); }
function cloudinaryPath(kind: MediaKind, conversationId: string, messageId: string) { return `cloudinary:${kind}:${conversationId}_${messageId}`; }
function cloudinaryPublicId(conversationId: string, messageId: string) { return `dislight/chat-media/${conversationId}_${messageId}`; }
function parseCloudinaryPath(path: string) {
  const match = /^cloudinary:(image|video):([0-9a-f-]{36})_([0-9a-f-]{36})$/i.exec(path);
  return match && uuid(match[2]) && uuid(match[3]) ? { kind: match[1] as MediaKind, conversationId: match[2], messageId: match[3] } : null;
}
function objectSize(object: StorageObject): number { const size = Number(object.metadata?.size); return Number.isFinite(size) && size >= 0 ? size : LEGACY_MAX_UPLOAD_BYTES; }
function objectCreatedAt(object: StorageObject): number { const timestamp = object.created_at ? Date.parse(object.created_at) : 0; return Number.isFinite(timestamp) ? timestamp : 0; }

async function sha1(value: string) {
  const bytes = new TextEncoder().encode(value);
  const digest = await crypto.subtle.digest("SHA-1", bytes);
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

async function cloudinarySignature(parameters: Record<string, string | number>) {
  const secret = Deno.env.get("CLOUDINARY_API_SECRET");
  if (!secret) throw new Error("Cloudinary media delivery is not configured.");
  const serialized = Object.entries(parameters).sort(([a], [b]) => a.localeCompare(b)).map(([key, value]) => `${key}=${value}`).join("&");
  return sha1(`${serialized}${secret}`);
}

async function destroyCloudinaryAsset(kind: MediaKind, conversationId: string, messageId: string) {
  if (!cloudinaryReady()) return false;
  const timestamp = Math.floor(Date.now() / 1000);
  const publicId = cloudinaryPublicId(conversationId, messageId);
  const signature = await cloudinarySignature({ invalidate: "true", public_id: publicId, timestamp });
  const form = new FormData();
  form.set("api_key", Deno.env.get("CLOUDINARY_API_KEY")!);
  form.set("public_id", publicId);
  form.set("timestamp", String(timestamp));
  form.set("invalidate", "true");
  form.set("signature", signature);
  const response = await fetch(`https://api.cloudinary.com/v1_1/${CLOUDINARY_CLOUD_NAME}/${kind}/destroy`, { method: "POST", body: form });
  const body = await response.json().catch(() => null) as { result?: string } | null;
  return response.ok && (body?.result === "ok" || body?.result === "not found");
}

function createAdminClient() {
  const url = Deno.env.get("SUPABASE_URL");
  const namedSecrets = JSON.parse(Deno.env.get("SUPABASE_SECRET_KEYS") ?? "{}");
  const key = namedSecrets.default ?? Deno.env.get("SUPABASE_SERVICE_ROLE_KEY");
  if (!url || !key) throw new Error("Supabase admin credentials are unavailable");
  return createClient(url, key, { auth: { persistSession: false, autoRefreshToken: false } });
}

async function listAllChatMedia(supabaseAdmin: any): Promise<StorageObject[]> {
  const objects: StorageObject[] = []; let offset = 0;
  while (true) {
    const { data, error } = await supabaseAdmin.storage.from(CHAT_BUCKET).list("", { limit: LIST_PAGE_SIZE, offset, sortBy: { column: "created_at", order: "asc" } });
    if (error) throw new Error(`Unable to inspect chat storage: ${error.message}`);
    const page = (data ?? []).filter((item: StorageObject) => Boolean(item.name));
    objects.push(...page); if (page.length < LIST_PAGE_SIZE) break; offset += page.length;
  }
  return objects;
}
async function releaseReservations(supabaseAdmin: any, paths: string[]) { if (paths.length) { const { error } = await supabaseAdmin.rpc("release_chat_media_reservations", { p_paths: paths }); if (error) throw new Error(`Unable to release media reservations: ${error.message}`); } }
async function releaseCloudReservations(supabaseAdmin: any, paths: string[]) { if (paths.length) { const { error } = await supabaseAdmin.rpc("release_cloud_chat_media_reservations", { p_paths: paths }); if (error) throw new Error(`Unable to release cloud media reservations: ${error.message}`); } }

async function removeObjects(supabaseAdmin: any, paths: string[]) {
  const deletedAt = new Date().toISOString();
  for (let index = 0; index < paths.length; index += REMOVE_BATCH_SIZE) {
    const batch = paths.slice(index, index + REMOVE_BATCH_SIZE);
    const { error: removeError } = await supabaseAdmin.storage.from(CHAT_BUCKET).remove(batch);
    if (removeError) throw new Error(`Unable to purge chat media: ${removeError.message}`);
    const { error: updateError } = await supabaseAdmin.from("messages").update({ media_path: null, media_deleted_at: deletedAt }).in("media_path", batch);
    if (updateError) throw new Error(`Media was removed but message cleanup failed: ${updateError.message}`);
    await releaseReservations(supabaseAdmin, batch);
  }
}

async function cleanupStorageChatMedia(supabaseAdmin: any, reserveBytes: number) {
  const objects = await listAllChatMedia(supabaseAdmin); const cutoff = Date.now() - RETENTION_MS; const deleteNames = new Set<string>(); let remainingBytes = 0;
  for (const object of objects) { if (objectCreatedAt(object) <= cutoff) deleteNames.add(object.name); else remainingBytes += objectSize(object); }
  if (remainingBytes + reserveBytes > CHAT_MEDIA_BUDGET_BYTES) {
    let bytesToFree = remainingBytes + reserveBytes - CHAT_MEDIA_BUDGET_BYTES;
    for (const object of objects) { if (bytesToFree <= 0) break; if (deleteNames.has(object.name)) continue; deleteNames.add(object.name); bytesToFree -= objectSize(object); remainingBytes -= objectSize(object); }
  }
  const paths = [...deleteNames]; await removeObjects(supabaseAdmin, paths);
  return { deletedObjects: paths.length, remainingBytes: Math.max(0, remainingBytes), budgetBytes: CHAT_MEDIA_BUDGET_BYTES };
}

async function cleanupCloudinaryChatMedia(supabaseAdmin: any, reserveBytes: number) {
  const { data, error } = await supabaseAdmin.from("messages").select("id,media_path,media_kind,media_size_bytes,media_expires_at,created_at").like("media_path", "cloudinary:%").is("media_deleted_at", null).order("created_at", { ascending: true });
  if (error) throw new Error(`Unable to inspect Cloudinary media: ${error.message}`);
  const records = (data ?? []) as CloudMessage[]; const remove = new Set<string>(); let remainingBytes = 0; const now = Date.now();
  for (const record of records) { if (!record.media_expires_at || Date.parse(record.media_expires_at) <= now) remove.add(record.id); else remainingBytes += Number(record.media_size_bytes) || 0; }
  if (remainingBytes + reserveBytes > CLOUDINARY_CHAT_MEDIA_BUDGET_BYTES) {
    let bytesToFree = remainingBytes + reserveBytes - CLOUDINARY_CHAT_MEDIA_BUDGET_BYTES;
    for (const record of records) { if (bytesToFree <= 0) break; if (remove.has(record.id)) continue; remove.add(record.id); bytesToFree -= Number(record.media_size_bytes) || 0; remainingBytes -= Number(record.media_size_bytes) || 0; }
  }
  const deletedAt = new Date().toISOString(); let deletedObjects = 0;
  for (const record of records.filter((entry) => remove.has(entry.id))) {
    const parsed = parseCloudinaryPath(record.media_path); if (!parsed) continue;
    if (await destroyCloudinaryAsset(parsed.kind, parsed.conversationId, parsed.messageId)) {
      const { error: updateError } = await supabaseAdmin.from("messages").update({ media_path: null, media_deleted_at: deletedAt }).eq("id", record.id).eq("media_path", record.media_path);
      if (updateError) throw new Error(`Cloudinary media was removed but message cleanup failed: ${updateError.message}`);
      await releaseCloudReservations(supabaseAdmin, [record.media_path]); deletedObjects += 1;
    }
  }
  return { deletedObjects, remainingBytes: Math.max(0, remainingBytes), budgetBytes: CLOUDINARY_CHAT_MEDIA_BUDGET_BYTES };
}

async function userCanAccessConversation(supabase: any, conversationId: string) { const { data, error } = await supabase.from("conversations").select("id").eq("id", conversationId).maybeSingle(); return !error && Boolean(data); }

const authenticatedHandler = withSupabase({ auth: "user" }, async (req, ctx) => {
  const supabaseAdmin = ctx.supabaseAdmin as any;
  if (req.method !== "POST") return json({ error: "POST required" }, 405);
  const body = await req.json().catch(() => null) as RequestBody | null;
  if (!body) return json({ error: "Invalid JSON body" }, 400);
  if (!ctx.userClaims?.id) return json({ error: "A signed-in user is required" }, 401);
  if (body.mode === "capability") return json({ provider: cloudinaryReady() ? "cloudinary" : "storage" });

  if (body.mode === "reserve") {
    const validMedia = uuid(body.conversationId) && uuid(body.messageId) && Number.isFinite(body.sizeBytes) && Number(body.sizeBytes) >= 1 && (body.kind === "image" || body.kind === "video");
    if (!validMedia) return json({ error: "Invalid media reservation" }, 400);
    if (!(await userCanAccessConversation(ctx.supabase, body.conversationId!))) return json({ error: "Conversation not found" }, 404);

    const useCloudinary = body.provider === "cloudinary" && cloudinaryReady();
    if (useCloudinary) {
      const maxBytes = body.kind === "image" ? CLOUDINARY_IMAGE_MAX_BYTES : CLOUDINARY_VIDEO_MAX_BYTES;
      const validDelivery = (body.kind === "image" && body.mimeType === "image/webp") || (body.kind === "video" && body.mimeType === "video/mp4");
      if (!validDelivery || Number(body.sizeBytes) > maxBytes || !body.uploadMimeType?.startsWith(`${body.kind}/`)) return json({ error: "Invalid Cloudinary media upload" }, 400);
      await cleanupCloudinaryChatMedia(supabaseAdmin, Number(body.sizeBytes));
      const path = cloudinaryPath(body.kind!, body.conversationId!, body.messageId!);
      const { data: reserved, error: reserveError } = await supabaseAdmin.rpc("reserve_cloud_chat_media_upload", { p_path: path, p_conversation: body.conversationId, p_message: body.messageId, p_user: ctx.userClaims.id, p_bytes: Number(body.sizeBytes) });
      if (reserveError) return json({ error: reserveError.message }, 500);
      if (!reserved) return json({ error: "Chat media storage is full. The oldest attachments are being cleared; try again shortly." }, 409);
      const timestamp = Math.floor(Date.now() / 1000); const publicId = cloudinaryPublicId(body.conversationId!, body.messageId!);
      const signature = await cloudinarySignature({ public_id: publicId, timestamp });
      return json({ provider: "cloudinary", path, publicId, uploadUrl: `https://api.cloudinary.com/v1_1/${CLOUDINARY_CLOUD_NAME}/${body.kind}/upload`, fields: { api_key: Deno.env.get("CLOUDINARY_API_KEY")!, public_id: publicId, timestamp: String(timestamp), signature }, maxFileBytes: maxBytes });
    }

    if (!((body.kind === "image" && body.mimeType === "image/webp") || (body.kind === "video" && body.mimeType === "video/webm")) || Number(body.sizeBytes) > LEGACY_MAX_UPLOAD_BYTES) return json({ error: "Invalid legacy media upload" }, 400);
    await cleanupStorageChatMedia(supabaseAdmin, LEGACY_MAX_UPLOAD_BYTES);
    const extension = body.kind === "image" ? "webp" : "webm"; const path = `${body.conversationId}_${body.messageId}.${extension}`;
    const { data: reserved, error: reserveError } = await supabaseAdmin.rpc("reserve_chat_media_upload", { p_path: path, p_conversation: body.conversationId, p_message: body.messageId, p_user: ctx.userClaims.id });
    if (reserveError) return json({ error: reserveError.message }, 500); if (!reserved) return json({ error: "Chat storage is busy. Try again in a few minutes." }, 409);
    const { data, error } = await supabaseAdmin.storage.from(CHAT_BUCKET).createSignedUploadUrl(path);
    if (error || !data?.token) { await releaseReservations(supabaseAdmin, [path]); return json({ error: error?.message ?? "Unable to create upload URL" }, 500); }
    return json({ provider: "storage", path, token: data.token, maxFileBytes: LEGACY_MAX_UPLOAD_BYTES, expiresAfterHours: 72 });
  }

  if (body.mode === "discard" || body.mode === "finalize") {
    if (typeof body.path !== "string") return json({ error: "Path is required" }, 400);
    const cloud = parseCloudinaryPath(body.path);
    const legacy = body.path.match(/^([0-9a-f-]{36})_([0-9a-f-]{36})\.(webp|webm)$/i);
    const conversationId = cloud?.conversationId ?? legacy?.[1];
    if (!conversationId || !uuid(conversationId) || !(await userCanAccessConversation(ctx.supabase, conversationId))) return json({ error: "Conversation not found" }, 404);
    if (body.mode === "finalize") {
      const { data: message } = await supabaseAdmin.from("messages").select("id").eq("media_path", body.path).eq("sender_id", ctx.userClaims.id).maybeSingle();
      if (!message) return json({ error: "Media message not found" }, 404);
      if (cloud) await releaseCloudReservations(supabaseAdmin, [body.path]); else await releaseReservations(supabaseAdmin, [body.path]);
      return json({ finalized: true });
    }
    const { data: existingMessage } = await supabaseAdmin.from("messages").select("id").eq("media_path", body.path).maybeSingle();
    if (existingMessage) return json({ error: "Media is attached to a message" }, 409);
    if (cloud) { if (!(await destroyCloudinaryAsset(cloud.kind, cloud.conversationId, cloud.messageId))) return json({ error: "Unable to discard Cloudinary media" }, 500); await releaseCloudReservations(supabaseAdmin, [body.path]); }
    else { const { error } = await supabaseAdmin.storage.from(CHAT_BUCKET).remove([body.path]); if (error) return json({ error: error.message }, 500); await releaseReservations(supabaseAdmin, [body.path]); }
    return json({ discarded: true });
  }
  return json({ error: "Unknown cleanup mode" }, 400);
});

export default {
  async fetch(req: Request) {
    if (req.method === "OPTIONS") return new Response("ok", { headers: corsHeaders });
    const body = await req.clone().json().catch(() => null) as RequestBody | null;
    if (body?.mode === "scheduled") {
      const admin = createAdminClient();
      const storage = await cleanupStorageChatMedia(admin, 0);
      const cloudinary = cloudinaryReady() ? await cleanupCloudinaryChatMedia(admin, 0) : { deletedObjects: 0, remainingBytes: 0, budgetBytes: CLOUDINARY_CHAT_MEDIA_BUDGET_BYTES };
      return json({ storage, cloudinary });
    }
    return authenticatedHandler(req);
  },
};