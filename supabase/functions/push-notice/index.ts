import "@supabase/functions-js/edge-runtime.d.ts";
import { createClient } from "npm:@supabase/supabase-js@2.110.2";

// Deliberately not withSupabase({auth:"user"}) - this is meant to be curled
// directly from a terminal (see NOTICE_PUSH_SECRET), not called from the
// signed-in app, so there's no user JWT to check. The secret header is the
// only gate; verify_jwt is off at deploy time to match (same reasoning as
// tweet-video-proxy - a plain request can't attach an Authorization header
// the gateway would accept).

const corsHeaders = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Headers": "content-type, x-notice-secret",
  "Access-Control-Allow-Methods": "POST, OPTIONS",
};

const json = (body: unknown, status = 200) => Response.json(body, { status, headers: corsHeaders });
const SEVERITIES = new Set(["neutral", "warning", "danger"]);

Deno.serve(async (req) => {
  if (req.method === "OPTIONS") return new Response("ok", { headers: corsHeaders });
  if (req.method !== "POST") return json({ error: "POST required" }, 405);

  const secret = Deno.env.get("NOTICE_PUSH_SECRET");
  if (!secret || req.headers.get("x-notice-secret") !== secret) {
    return json({ error: "Unauthorized" }, 401);
  }

  const body = await req.json().catch(() => null) as { severity?: unknown; message?: unknown } | null;
  const message = typeof body?.message === "string" ? body.message.trim() : "";
  const severity = typeof body?.severity === "string" && SEVERITIES.has(body.severity) ? body.severity : "neutral";
  if (!message || message.length > 500) {
    return json({ error: "message is required (1-500 chars)" }, 400);
  }

  const url = Deno.env.get("SUPABASE_URL");
  const namedSecrets = JSON.parse(Deno.env.get("SUPABASE_SECRET_KEYS") ?? "{}");
  const key = namedSecrets.default ?? Deno.env.get("SUPABASE_SERVICE_ROLE_KEY");
  if (!url || !key) return json({ error: "Server misconfigured" }, 500);
  const admin = createClient(url, key, { auth: { persistSession: false, autoRefreshToken: false } });

  const { data, error } = await admin.from("app_notices").insert({ severity, message }).select().single();
  if (error) return json({ error: error.message }, 500);
  return json({ notice: data });
});
