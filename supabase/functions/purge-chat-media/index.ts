import "@supabase/functions-js/edge-runtime.d.ts";
import { withSupabase } from "@supabase/server";
import { createClient } from "npm:@supabase/supabase-js@2.110.2";

const CHAT_BUCKET = "chat-media";
const CHAT_MEDIA_BUDGET_BYTES = 512 * 1024 * 1024;
const MAX_UPLOAD_BYTES = 50 * 1024 * 1024;
const RETENTION_MS = 3 * 24 * 60 * 60 * 1000;
const LIST_PAGE_SIZE = 1000;
const REMOVE_BATCH_SIZE = 100;

const corsHeaders = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Headers": "authorization, apikey, content-type, x-client-info",
  "Access-Control-Allow-Methods": "POST, OPTIONS",
};

type StorageObject = {
  name: string;
  created_at?: string | null;
  metadata?: { size?: number | string } | null;
};

type RequestBody = {
  mode?: "scheduled" | "reserve" | "discard" | "finalize";
  conversationId?: string;
  messageId?: string;
  path?: string;
  kind?: "image" | "video";
  mimeType?: "image/webp" | "video/webm";
  sizeBytes?: number;
};

function json(body: unknown, status = 200) {
  return Response.json(body, { status, headers: corsHeaders });
}

function createAdminClient() {
  const url = Deno.env.get("SUPABASE_URL");
  const namedSecrets = JSON.parse(Deno.env.get("SUPABASE_SECRET_KEYS") ?? "{}");
  const key = namedSecrets.default ?? Deno.env.get("SUPABASE_SERVICE_ROLE_KEY");
  if (!url || !key) throw new Error("Supabase admin credentials are unavailable");
  return createClient(url, key, {
    auth: { persistSession: false, autoRefreshToken: false },
  });
}

function uuid(value: unknown): value is string {
  return typeof value === "string" &&
    /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value);
}

function objectSize(object: StorageObject): number {
  const size = Number(object.metadata?.size);
  return Number.isFinite(size) && size >= 0 ? size : MAX_UPLOAD_BYTES;
}

function objectCreatedAt(object: StorageObject): number {
  const timestamp = object.created_at ? Date.parse(object.created_at) : 0;
  return Number.isFinite(timestamp) ? timestamp : 0;
}

async function listAllChatMedia(supabaseAdmin: any): Promise<StorageObject[]> {
  const objects: StorageObject[] = [];
  let offset = 0;

  while (true) {
    const { data, error } = await supabaseAdmin.storage.from(CHAT_BUCKET).list("", {
      limit: LIST_PAGE_SIZE,
      offset,
      sortBy: { column: "created_at", order: "asc" },
    });
    if (error) throw new Error(`Unable to inspect chat storage: ${error.message}`);

    const page = (data ?? []).filter((item: StorageObject) => Boolean(item.name));
    objects.push(...page);
    if (page.length < LIST_PAGE_SIZE) break;
    offset += page.length;
  }

  return objects;
}

async function releaseReservations(supabaseAdmin: any, paths: string[]) {
  if (paths.length === 0) return;
  const { error } = await supabaseAdmin.rpc("release_chat_media_reservations", {
    p_paths: paths,
  });
  if (error) throw new Error(`Unable to release media reservations: ${error.message}`);
}

async function removeObjects(supabaseAdmin: any, paths: string[]) {
  const deletedAt = new Date().toISOString();

  for (let index = 0; index < paths.length; index += REMOVE_BATCH_SIZE) {
    const batch = paths.slice(index, index + REMOVE_BATCH_SIZE);
    const { error: removeError } = await supabaseAdmin.storage
      .from(CHAT_BUCKET)
      .remove(batch);
    if (removeError) throw new Error(`Unable to purge chat media: ${removeError.message}`);

    let updateError: { message: string } | null = null;
    for (let attempt = 0; attempt < 3; attempt += 1) {
      const result = await supabaseAdmin
        .from("messages")
        .update({ media_path: null, media_deleted_at: deletedAt })
        .in("media_path", batch);
      updateError = result.error;
      if (!updateError) break;
    }
    if (updateError) {
      throw new Error(`Media was removed but message cleanup failed: ${updateError.message}`);
    }

    await releaseReservations(supabaseAdmin, batch);
  }
}

async function cleanupChatMedia(supabaseAdmin: any, reserveBytes: number) {
  const objects = await listAllChatMedia(supabaseAdmin);
  const cutoff = Date.now() - RETENTION_MS;
  const deleteNames = new Set<string>();
  let remainingBytes = 0;

  for (const object of objects) {
    if (objectCreatedAt(object) <= cutoff) deleteNames.add(object.name);
    else remainingBytes += objectSize(object);
  }

  if (remainingBytes + reserveBytes > CHAT_MEDIA_BUDGET_BYTES) {
    let bytesToFree = remainingBytes + reserveBytes - CHAT_MEDIA_BUDGET_BYTES;
    for (const object of objects) {
      if (bytesToFree <= 0) break;
      if (deleteNames.has(object.name)) continue;
      deleteNames.add(object.name);
      bytesToFree -= objectSize(object);
      remainingBytes -= objectSize(object);
    }
  }

  const paths = [...deleteNames];
  await removeObjects(supabaseAdmin, paths);

  return {
    deletedObjects: paths.length,
    remainingBytes: Math.max(0, remainingBytes),
    budgetBytes: CHAT_MEDIA_BUDGET_BYTES,
  };
}

async function userCanAccessConversation(supabase: any, conversationId: string) {
  const { data, error } = await supabase
    .from("conversations")
    .select("id")
    .eq("id", conversationId)
    .maybeSingle();
  return !error && Boolean(data);
}

const authenticatedHandler = withSupabase(
  { auth: "user" },
  async (req, ctx) => {
    const supabaseAdmin = ctx.supabaseAdmin as any;
    if (req.method !== "POST") return json({ error: "POST required" }, 405);

    let body: RequestBody;
    try {
      body = await req.json();
    } catch {
      return json({ error: "Invalid JSON body" }, 400);
    }


    if (!ctx.userClaims?.id) {
      return json({ error: "A signed-in user is required" }, 401);
    }

    if (body.mode === "reserve") {
      if (
        !uuid(body.conversationId) ||
        !uuid(body.messageId) ||
        !Number.isFinite(body.sizeBytes) ||
        Number(body.sizeBytes) < 1 ||
        Number(body.sizeBytes) > MAX_UPLOAD_BYTES ||
        !((body.kind === "image" && body.mimeType === "image/webp") ||
          (body.kind === "video" && body.mimeType === "video/webm"))
      ) {
        return json({ error: "Invalid media reservation" }, 400);
      }

      if (!(await userCanAccessConversation(ctx.supabase, body.conversationId))) {
        return json({ error: "Conversation not found" }, 404);
      }

      await cleanupChatMedia(supabaseAdmin, MAX_UPLOAD_BYTES);

      const extension = body.kind === "image" ? "webp" : "webm";
      const path = `${body.conversationId}_${body.messageId}.${extension}`;
      const { data: reserved, error: reserveError } = await supabaseAdmin.rpc(
        "reserve_chat_media_upload",
        {
          p_path: path,
          p_conversation: body.conversationId,
          p_message: body.messageId,
          p_user: ctx.userClaims.id,
        }
      );

      if (reserveError) return json({ error: reserveError.message }, 500);
      if (!reserved) {
        return json({ error: "Chat storage is busy. Try again in a few minutes." }, 409);
      }

      const { data, error } = await supabaseAdmin.storage
        .from(CHAT_BUCKET)
        .createSignedUploadUrl(path);
      if (error || !data?.token) {
        await releaseReservations(supabaseAdmin, [path]);
        return json({ error: error?.message ?? "Unable to create upload URL" }, 500);
      }

      return json({
        path,
        token: data.token,
        maxFileBytes: MAX_UPLOAD_BYTES,
        expiresAfterHours: 72,
      });
    }

    if (body.mode === "discard" || body.mode === "finalize") {
      if (typeof body.path !== "string") return json({ error: "Path is required" }, 400);
      const match = body.path.match(
        /^([0-9a-f-]{36})_([0-9a-f-]{36})\.(webp|webm)$/i
      );
      if (!match || !uuid(match[1]) || !uuid(match[2])) {
        return json({ error: "Invalid media path" }, 400);
      }
      const conversationId = match[1];
      if (!(await userCanAccessConversation(ctx.supabase, conversationId))) {
        return json({ error: "Conversation not found" }, 404);
      }

      if (body.mode === "finalize") {
        const { data: message } = await supabaseAdmin
          .from("messages")
          .select("id")
          .eq("media_path", body.path)
          .eq("sender_id", ctx.userClaims.id)
          .maybeSingle();
        if (!message) return json({ error: "Media message not found" }, 404);
        await releaseReservations(supabaseAdmin, [body.path]);
        return json({ finalized: true });
      }

      const { data: existingMessage } = await supabaseAdmin
        .from("messages")
        .select("id")
        .eq("media_path", body.path)
        .maybeSingle();
      if (existingMessage) return json({ error: "Media is attached to a message" }, 409);

      const { error: removeError } = await supabaseAdmin.storage
        .from(CHAT_BUCKET)
        .remove([body.path]);
      if (removeError) return json({ error: removeError.message }, 500);
      await releaseReservations(supabaseAdmin, [body.path]);
      return json({ discarded: true });
    }

    return json({ error: "Unknown cleanup mode" }, 400);
  }
);

export default {
  async fetch(req: Request) {
    if (req.method === "OPTIONS") {
      return new Response("ok", { headers: corsHeaders });
    }

    const body = await req.clone().json().catch(() => null) as RequestBody | null;
    if (body?.mode === "scheduled") {
      return json(await cleanupChatMedia(createAdminClient(), 0));
    }

    return authenticatedHandler(req);
  },
};


