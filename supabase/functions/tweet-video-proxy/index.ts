import "jsr:@supabase/functions-js/edge-runtime.d.ts";
import { createClient } from "npm:@supabase/supabase-js@2.110.2";
import { allowRequest } from "../_shared/quota.ts";

// video.twimg.com 403s a plain client-side fetch/<video src> - it checks
// for a Referer the browser Fetch API forbids scripts from ever setting,
// so no client-side fix is possible. This proxies the request server-side
// instead, where that restriction doesn't apply, and forwards the
// incoming Range header so native <video> scrubbing still works.
// Public (no JWT) on purpose: a plain <video src="..."> GET can't attach
// an Authorization header. Scoped tightly to Twitter/X's own media CDN
// hostnames instead, so this can't be used as an open proxy for arbitrary
// URLs.
const ALLOWED_HOSTS = new Set(["video.twimg.com", "pbs.twimg.com"]);
const UPSTREAM_USER_AGENT =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36";

function corsHeaders(extra: Record<string, string> = {}): Record<string, string> {
  return {
    "Access-Control-Allow-Origin": "*",
    "Access-Control-Allow-Headers": "range, content-type",
    "Access-Control-Allow-Methods": "GET, HEAD, OPTIONS",
    "Access-Control-Expose-Headers": "content-range, accept-ranges, content-length",
    ...extra,
  };
}

Deno.serve(async (req) => {
  if (req.method === "OPTIONS") return new Response("ok", { headers: corsHeaders() });
  if (req.method !== "GET" && req.method !== "HEAD") {
    return new Response("Method not allowed", { status: 405, headers: corsHeaders() });
  }

  const secrets = JSON.parse(Deno.env.get("SUPABASE_SECRET_KEYS") ?? "{}");
  const admin = createClient(Deno.env.get("SUPABASE_URL")!, secrets.default ?? Deno.env.get("SUPABASE_SERVICE_ROLE_KEY")!, { auth: { persistSession: false } });
  const address = req.headers.get("x-forwarded-for")?.split(",")[0]?.trim() ?? "unknown";
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(address));
  const key = Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2,"0")).join("");
  if (!(await allowRequest(admin, key, "tweet-video", 60)) || !(await allowRequest(admin, "global", "tweet-video", 5000, 86400))) return new Response("Rate limit exceeded", { status: 429, headers: corsHeaders({ "retry-after": "60" }) });
  const target = new URL(req.url).searchParams.get("url");
  if (!target) return new Response("Missing url", { status: 400, headers: corsHeaders() });

  let parsed: URL;
  try {
    parsed = new URL(target);
  } catch {
    return new Response("Invalid url", { status: 400, headers: corsHeaders() });
  }
  if (parsed.protocol !== "https:" || !ALLOWED_HOSTS.has(parsed.hostname) || parsed.username || parsed.password || (parsed.port && parsed.port !== "443")) {
    return new Response("Host not allowed", { status: 400, headers: corsHeaders() });
  }

  const upstreamHeaders: Record<string, string> = {
    "user-agent": UPSTREAM_USER_AGENT,
    referer: "https://x.com/",
  };
  const range = req.headers.get("range");
  if (range) upstreamHeaders.range = range;

  let upstream: Response;
  try {
    upstream = await fetch(parsed, { headers: upstreamHeaders, method: req.method, redirect: "error", signal: AbortSignal.timeout(30_000) });
  } catch {
    return new Response("Upstream fetch failed", { status: 502, headers: corsHeaders() });
  }

  if (!upstream.ok) {
    return new Response("Upstream error", { status: upstream.status, headers: corsHeaders() });
  }

  // Cache aggressively - the content behind a given twimg video URL never
  // changes, so this is a prime candidate for the browser's own HTTP cache.
  // Without an explicit Cache-Control, the browser has no caching hint at
  // all (no Last-Modified/ETag forwarded either), which means every replay
  // or scroll-back could re-proxy the entire video through this function -
  // real egress cost on every repeat view of the same tweet, not just once.
  const headers = corsHeaders({
    "content-type": upstream.headers.get("content-type") ?? "application/octet-stream",
    "cache-control": "public, max-age=604800, immutable",
  });
  const contentLength = upstream.headers.get("content-length");
  if (Number(contentLength) > 100 * 1024 * 1024) { await upstream.body?.cancel(); return new Response("Media too large", { status: 413, headers: corsHeaders() }); }
  if (contentLength) headers["content-length"] = contentLength;
  const contentRange = upstream.headers.get("content-range");
  if (contentRange) headers["content-range"] = contentRange;
  const acceptRanges = upstream.headers.get("accept-ranges");
  if (acceptRanges) headers["accept-ranges"] = acceptRanges;

  let transferred = 0;
  const boundedBody = upstream.body?.pipeThrough(new TransformStream<Uint8Array, Uint8Array>({
    transform(chunk, controller) {
      transferred += chunk.byteLength;
      if (transferred > 100 * 1024 * 1024) controller.error(new Error("Media exceeds transfer limit")); else controller.enqueue(chunk);
    },
  }));
  return new Response(boundedBody, { status: upstream.status, headers });
});
