import "@supabase/functions-js/edge-runtime.d.ts";
import { withSupabase } from "@supabase/server";
import { safeHttpUrl as isSafeHttpUrl, fetchPublicDocument } from "../_shared/safe-http.ts";
import { allowRequest } from "../_shared/quota.ts";

const corsHeaders = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Headers": "authorization, apikey, content-type, x-client-info",
  "Access-Control-Allow-Methods": "POST, OPTIONS",
};
const MAX_HTML_BYTES = 384 * 1024;
const FETCH_TIMEOUT_MS = 5_000;

type SitePreview = {
  kind: "site";
  url: string;
  title: string;
  description?: string;
  siteName?: string;
  image?: string;
};

type TweetMedia = {
  type: "photo" | "video" | "gif";
  url: string;
  thumbnailUrl?: string;
  width?: number;
  height?: number;
};

type TweetPreview = {
  kind: "tweet";
  url: string;
  author: { name: string; handle: string; avatarUrl?: string };
  text: string;
  media: TweetMedia[];
};

type YouTubePreview = {
  kind: "youtube";
  url: string;
  videoId: string;
  title: string;
  authorName?: string;
  thumbnail: string;
};

type LinkPreview = SitePreview | TweetPreview | YouTubePreview;

function json(body: unknown, status = 200) {
  return Response.json(body, { status, headers: corsHeaders });
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

async function fetchDocument(initialUrl: URL) {
  let current = initialUrl;
  for (let redirects = 0; redirects <= 3; redirects += 1) {
    const response = await fetchPublicDocument(current, MAX_HTML_BYTES, FETCH_TIMEOUT_MS);
    if (response.status >= 300 && response.status < 400) {
      const next = response.headers["location"];
      const resolved = next ? isSafeHttpUrl(next, current.href) : null;
      if (!resolved) throw new Error("Unsafe redirect.");
      current = resolved;
      continue;
    }
    if (response.status < 200 || response.status >= 300) throw new Error("Page could not be fetched.");
    if (!response.headers["content-type"]?.toLowerCase().includes("text/html")) throw new Error("Page is not HTML.");
    return { url: current, html: response.html };
  }
  throw new Error("Too many redirects.");
}

function isTweetUrl(url: URL) {
  const hostname = url.hostname.toLowerCase().replace(/^(www|mobile)\./, "");
  if (hostname !== "twitter.com" && hostname !== "x.com") return false;
  return /^\/[^/]+\/status\/\d+/.test(url.pathname) || /^\/i\/status\/\d+/.test(url.pathname);
}

const YOUTUBE_ID = /^[a-zA-Z0-9_-]{11}$/;

// Covers youtube.com/watch?v=, youtu.be/, /shorts/, and /embed/ - anything
// else (playlists, channel pages, music.youtube.com) falls through to the
// generic OG scrape below instead of being treated as a single video.
function extractYouTubeVideoId(url: URL): string | null {
  const hostname = url.hostname.toLowerCase().replace(/^(www|m|music)\./, "");
  if (hostname === "youtu.be") {
    const id = url.pathname.slice(1).split("/")[0];
    return id && YOUTUBE_ID.test(id) ? id : null;
  }
  if (hostname !== "youtube.com") return null;
  if (url.pathname === "/watch") {
    const id = url.searchParams.get("v");
    return id && YOUTUBE_ID.test(id) ? id : null;
  }
  const shortsMatch = url.pathname.match(/^\/shorts\/([a-zA-Z0-9_-]{11})/);
  if (shortsMatch) return shortsMatch[1];
  const embedMatch = url.pathname.match(/^\/embed\/([a-zA-Z0-9_-]{11})/);
  if (embedMatch) return embedMatch[1];
  return null;
}

// YouTube's oEmbed endpoint is public, unauthenticated, and CORS-friendly -
// no API key needed, just title/author for a preview card. The actual
// player is a plain <iframe src=".../embed/ID">, which YouTube officially
// supports for cross-site embedding (unlike Twitter's video CDN, no
// Referer fight, no proxy needed).
async function fetchYouTubePreview(url: URL, videoId: string): Promise<YouTubePreview | null> {
  try {
    const oembedUrl = `https://www.youtube.com/oembed?url=${encodeURIComponent(`https://www.youtube.com/watch?v=${videoId}`)}&format=json`;
    const response = await fetch(oembedUrl, {
      signal: AbortSignal.timeout(FETCH_TIMEOUT_MS),
      headers: { "user-agent": "Nitro Link Preview/1.0", accept: "application/json" },
    });
    if (!response.ok) return null;
    const data = await response.json();
    if (typeof data?.title !== "string") return null;
    return {
      kind: "youtube",
      url: url.href,
      videoId,
      title: text(data.title, 200),
      authorName: typeof data.author_name === "string" ? text(data.author_name, 80) : undefined,
      thumbnail: `https://i.ytimg.com/vi/${videoId}/hqdefault.jpg`,
    };
  } catch (error) {
    console.warn("YouTube preview unavailable", error instanceof Error ? error.message : "unknown");
    return null;
  }
}

// twitter.com/x.com serve almost no usable markup to a logged-out fetch, so a
// generic OG scrape can't recover tweet text, the author, or media. fxtwitter
// (the open-source "FixTweet" project) exposes exactly that as JSON, keyed
// off the same /user/status/id path shape - no API key needed.
async function fetchTweetPreview(url: URL): Promise<TweetPreview | null> {
  try {
    const statusId = url.pathname.match(/\/status\/(\d{1,25})(?:\/|$)/)?.[1];
    if (!statusId) return null;
    // Send only the public status identifier, never the original link, handle, query or fragment.
    const apiUrl = `https://api.fxtwitter.com/status/${statusId}`;
    const response = await fetch(apiUrl, {
      signal: AbortSignal.timeout(FETCH_TIMEOUT_MS),
      headers: { "user-agent": "Nitro Link Preview/1.0", accept: "application/json" },
    });
    if (!response.ok) return null;
    const data = await response.json();
    const tweet = data?.tweet;
    if (!tweet || typeof tweet.text !== "string" || !tweet.author?.screen_name) return null;

    const media: TweetMedia[] = [];
    for (const photo of tweet.media?.photos ?? []) {
      const safe = typeof photo?.url === "string" ? isSafeHttpUrl(photo.url) : null;
      if (safe?.protocol !== "https:") continue;
      media.push({ type: "photo", url: safe.href, width: photo.width, height: photo.height });
    }
    for (const video of tweet.media?.videos ?? []) {
      const safe = typeof video?.url === "string" ? isSafeHttpUrl(video.url) : null;
      if (safe?.protocol !== "https:") continue;
      const thumb = typeof video?.thumbnail_url === "string" ? isSafeHttpUrl(video.thumbnail_url) : null;
      media.push({
        type: video.type === "gif" ? "gif" : "video",
        url: safe.href,
        thumbnailUrl: thumb?.protocol === "https:" ? thumb.href : undefined,
        width: video.width,
        height: video.height,
      });
    }

    const avatar = typeof tweet.author.avatar_url === "string" ? isSafeHttpUrl(tweet.author.avatar_url) : null;
    const tweetUrl = typeof tweet.url === "string" ? isSafeHttpUrl(tweet.url) : null;
    return {
      kind: "tweet",
      url: tweetUrl?.href ?? url.href,
      author: {
        name: text(tweet.author.name ?? "", 80) || tweet.author.screen_name,
        handle: tweet.author.screen_name,
        avatarUrl: avatar?.protocol === "https:" ? avatar.href : undefined,
      },
      text: text(tweet.text, 600),
      media: media.slice(0, 4),
    };
  } catch (error) {
    console.warn("Tweet preview unavailable", error instanceof Error ? error.message : "unknown");
    return null;
  }
}

const handler = withSupabase({ auth: "user" }, async (req, ctx) => {
  if (req.method === "OPTIONS") return new Response("ok", { headers: corsHeaders });
  if (req.method !== "POST") return json({ error: "POST required" }, 405);
  const body = await req.json().catch(() => null) as { url?: unknown } | null;
  if (typeof body?.url !== "string" || body.url.length > 2_048) return json({ error: "Invalid URL" }, 400);
  if (!(await allowRequest(ctx.supabaseAdmin, ctx.userClaims!.id, "link-preview", 30))) return json({ error: "Too many previews" }, 429);
  const requestedUrl = isSafeHttpUrl(body.url);
  if (!requestedUrl) return json({ error: "Unsupported URL" }, 400);

  if (isTweetUrl(requestedUrl)) {
    const tweetPreview = await fetchTweetPreview(requestedUrl);
    if (tweetPreview) return json({ preview: tweetPreview });
    // Fall through to the generic scrape below as a fallback.
  }

  const youtubeVideoId = extractYouTubeVideoId(requestedUrl);
  if (youtubeVideoId) {
    const youtubePreview = await fetchYouTubePreview(requestedUrl, youtubeVideoId);
    if (youtubePreview) return json({ preview: youtubePreview });
    // Fall through to the generic scrape below as a fallback.
  }

  try {
    const { url, html } = await fetchDocument(requestedUrl);
    const ogTitle = meta(html, ["og:title", "twitter:title"]);
    const pageTitle = text(html.match(/<title\b[^>]*>([\s\S]*?)<\/title>/i)?.[1] ?? "", 180);
    const title = text(ogTitle || pageTitle || url.hostname, 180);
    const description = text(meta(html, ["og:description", "twitter:description", "description"]), 300);
    const siteName = text(meta(html, ["og:site_name"]), 80);
    const imageValue = meta(html, ["og:image", "twitter:image"]);
    const imageUrl = imageValue ? isSafeHttpUrl(imageValue, url.href) : null;
    const preview: SitePreview = { kind: "site", url: url.href, title };
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
