import { supabase } from "@/lib/supabase";

export type TweetMediaItem = { type: "photo" | "video" | "gif"; url: string; thumbnailUrl?: string };
export type LinkPreview =
  | {
      kind: "tweet";
      url: string;
      text?: string;
      author: { name: string; handle: string; avatarUrl?: string };
      media: TweetMediaItem[];
    }
  | { kind: "youtube"; url: string; videoId: string; title: string; authorName?: string; thumbnail: string }
  | { kind: "site"; url: string; title: string; description?: string; siteName?: string; image?: string };

const URL_PART = /(https?:\/\/[^\s<]+)/gi;

function stripTrailingPunctuation(value: string): string {
  const match = value.match(/^(.*?)([.,!?;:]+)$/);
  return match ? match[1] : value;
}

export function extractFirstUrl(content: string): string | null {
  const first = content.match(URL_PART)?.[0];
  return first ? stripTrailingPunctuation(first) : null;
}

const cache = new Map<string, LinkPreview | null>();

// undefined = never fetched, null = fetched but no preview available.
export function cachedLinkPreview(url: string): LinkPreview | null | undefined {
  return cache.get(url);
}

export async function resolveLinkPreview(url: string): Promise<LinkPreview | null> {
  if (cache.has(url)) return cache.get(url) ?? null;
  const { data, error } = await supabase.functions.invoke("link-preview", { body: { url } });
  const preview = !error && data?.preview ? data.preview : null;
  cache.set(url, preview);
  return preview;
}

// video.twimg.com 403s a plain browser fetch/<video src> - it checks for a
// Referer that the Fetch spec forbids scripts from ever setting, so there's
// no client-side fix. tweet-video-proxy (a Supabase Edge Function) fetches
// it server-side with that header instead, where the restriction doesn't
// apply, and forwards Range requests so native scrubbing still works.
export function tweetVideoProxyUrl(url: string): string {
  return `${import.meta.env.VITE_SUPABASE_URL}/functions/v1/tweet-video-proxy?url=${encodeURIComponent(url)}`;
}
