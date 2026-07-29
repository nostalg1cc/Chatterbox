import "@supabase/functions-js/edge-runtime.d.ts";
import { withSupabase } from "@supabase/server";

const corsHeaders = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Headers": "authorization, apikey, content-type, x-client-info",
  "Access-Control-Allow-Methods": "POST, OPTIONS",
};
const MAX_HTML_BYTES = 384 * 1024;
const FETCH_TIMEOUT_MS = 5_000;

type LinkPreview = {
  url: string;
  title: string;
  description?: string;
  siteName?: string;
  image?: string;
};

function json(body: unknown, status = 200) {
  return Response.json(body, { status, headers: corsHeaders });
}

function isPrivateIpv4(hostname: string) {
  const parts = hostname.split(".").map(Number);
  if (parts.length !== 4 || parts.some((part) => !Number.isInteger(part) || part < 0 || part > 255)) return false;
  return parts[0] === 0 || parts[0] === 10 || parts[0] === 127 || parts[0] === 169 && parts[1] === 254 || parts[0] === 172 && parts[1] >= 16 && parts[1] <= 31 || parts[0] === 192 && parts[1] === 168;
}

function isSafeHttpUrl(value: string, base?: string) {
  try {
    const url = new URL(value, base);
    const hostname = url.hostname.toLowerCase();
    const isIpv6 = hostname.includes(":");
    if (url.protocol !== "http:" && url.protocol !== "https:") return null;
    if (!hostname || hostname === "localhost" || hostname.endsWith(".localhost") || hostname.endsWith(".local") || hostname.endsWith(".internal") || hostname.endsWith(".home.arpa") || hostname === "metadata.google.internal" || isPrivateIpv4(hostname)) return null;
    if (isIpv6 && (hostname === "::1" || hostname.startsWith("fc") || hostname.startsWith("fd") || hostname.startsWith("fe80"))) return null;
    if (url.username || url.password) return null;
    return url;
  } catch {
    return null;
  }
}

function decodeHtml(value: string) {
  return value
    .replace(/&amp;/gi, "&")
    .replace(/&quot;/gi, "\"")
    .replace(/&#39;|&apos;/gi, "'")
    .replace(/&lt;/gi, "<")
    .replace(/&gt;/gi, ">")
    .replace(/\s+/g, " ")
    .trim();
}

function attr(tag: string, name: string) {
  const match = tag.match(new RegExp(`\\b${name}\\s*=\\s*(?:\"([^\"]*)\"|'([^']*)'|([^\\s>]+))`, "i"));
  return match?.[1] ?? match?.[2] ?? match?.[3] ?? "";
}

function meta(html: string, names: string[]) {
  const tags = html.match(/<meta\b[^>]*>/gi) ?? [];
  for (const tag of tags) {
    const key = (attr(tag, "property") || attr(tag, "name")).toLowerCase();
    if (names.includes(key)) {
      const value = decodeHtml(attr(tag, "content"));
      if (value) return value;
    }
  }
  return "";
}

function text(value: string, max: number) {
  return decodeHtml(value.replace(/<[^>]+>/g, "")).slice(0, max);
}

async function readHtml(response: Response) {
  const length = Number(response.headers.get("content-length") ?? 0);
  if (Number.isFinite(length) && length > MAX_HTML_BYTES) throw new Error("Page is too large.");
  if (!response.body) return "";
  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let total = 0;
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    total += value.byteLength;
    if (total > MAX_HTML_BYTES) {
      await reader.cancel();
      throw new Error("Page is too large.");
    }
    chunks.push(value);
  }
  const merged = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    merged.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return new TextDecoder().decode(merged);
}

async function fetchDocument(initialUrl: URL) {
  let current = initialUrl;
  for (let redirects = 0; redirects <= 3; redirects += 1) {
    const response = await fetch(current, {
      redirect: "manual",
      signal: AbortSignal.timeout(FETCH_TIMEOUT_MS),
      headers: { "user-agent": "Dislight Link Preview/1.0", accept: "text/html,application/xhtml+xml" },
    });
    if (response.status >= 300 && response.status < 400) {
      const next = response.headers.get("location");
      const resolved = next ? isSafeHttpUrl(next, current.href) : null;
      if (!resolved) throw new Error("Unsafe redirect.");
      current = resolved;
      continue;
    }
    if (!response.ok) throw new Error("Page could not be fetched.");
    if (!response.headers.get("content-type")?.toLowerCase().includes("text/html")) throw new Error("Page is not HTML.");
    return { url: current, html: await readHtml(response) };
  }
  throw new Error("Too many redirects.");
}

const handler = withSupabase({ auth: "user" }, async (req) => {
  if (req.method === "OPTIONS") return new Response("ok", { headers: corsHeaders });
  if (req.method !== "POST") return json({ error: "POST required" }, 405);
  const body = await req.json().catch(() => null) as { url?: unknown } | null;
  if (typeof body?.url !== "string" || body.url.length > 2_048) return json({ error: "Invalid URL" }, 400);
  const requestedUrl = isSafeHttpUrl(body.url);
  if (!requestedUrl) return json({ error: "Unsupported URL" }, 400);

  try {
    const { url, html } = await fetchDocument(requestedUrl);
    const ogTitle = meta(html, ["og:title", "twitter:title"]);
    const pageTitle = text(html.match(/<title\b[^>]*>([\s\S]*?)<\/title>/i)?.[1] ?? "", 180);
    const title = text(ogTitle || pageTitle || url.hostname, 180);
    const description = text(meta(html, ["og:description", "twitter:description", "description"]), 300);
    const siteName = text(meta(html, ["og:site_name"]), 80);
    const imageValue = meta(html, ["og:image", "twitter:image"]);
    const imageUrl = imageValue ? isSafeHttpUrl(imageValue, url.href) : null;
    const preview: LinkPreview = { url: url.href, title };
    if (description) preview.description = description;
    if (siteName) preview.siteName = siteName;
    if (imageUrl?.protocol === "https:") preview.image = imageUrl.href;
    return json({ preview });
  } catch (error) {
    // A preview failure is expected for many sites. Return a successful null
    // response so clients do not treat ordinary upstream refusal as an app error.
    console.warn("Link preview unavailable", error instanceof Error ? error.message : "unknown");
    return json({ preview: null });
  }
});

Deno.serve(handler);