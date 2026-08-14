import { useEffect, useMemo, useState } from "react";
import { motion } from "framer-motion";
import { isTauri } from "@/lib/tauri";
import { openUrl } from "@tauri-apps/plugin-opener";
import { supabase } from "@/lib/supabase";
import { useAlerts } from "@/stores/alerts";
import { useLightbox } from "@/stores/lightbox";

const URL_PART = /(https?:\/\/[^\s<]+)/gi;
const URL_ONLY = /^https?:\/\/[^\s<]+$/i;
const cache = new Map();
const videoBlobCache = new Map();

function splitTrailingPunctuation(value) {
  const match = value.match(/^(.*?)([.,!?;:]+)$/);
  return match ? { url: match[1], suffix: match[2] } : { url: value, suffix: "" };
}

async function reallyOpenLink(href) {
  if (isTauri) {
    await openUrl(href);
    return;
  }
  window.open(href, "_blank", "noopener,noreferrer");
}

// A whole tweet/link embed card is one big click target, so a stray click
// anywhere on it would otherwise jump straight out to the browser - confirm
// first instead, same for the smaller inline text links.
function openLink(url) {
  const safe = new URL(url);
  if (!/^https?:$/.test(safe.protocol)) throw new Error("Only web links are supported.");
  useAlerts.getState().show({
    severity: "neutral",
    message: `Open ${safe.hostname} in your browser?`,
    actions: [
      { label: "Dismiss" },
      { label: "Yes", confirm: true, onClick: () => void reallyOpenLink(safe.href) },
    ],
  });
}

function LinkText({ content, hidden }) {
  return (
    <>
      {content.split(URL_PART).map((part, index) => {
        if (!URL_ONLY.test(part)) return part;
        const { url, suffix } = splitTrailingPunctuation(part);
        if (url === hidden) return suffix || null;
        return (
          <span key={index}>
            <a
              className="v3-chat-link"
              href={url}
              target="_blank"
              rel="noreferrer"
              onClick={(event) => {
                event.preventDefault();
                void openLink(url);
              }}
            >
              {url}
            </a>
            {suffix}
          </span>
        );
      })}
    </>
  );
}

function XIcon(props) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true" {...props}>
      <path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z" />
    </svg>
  );
}

function TweetVideo({ url, poster, loop }) {
  // Twitter's video CDN advertises Range support (Accept-Ranges: bytes) but
  // doesn't actually honor Range requests - always returns the full file
  // with 200, never 206 Partial Content. A native <video src> tries a
  // range-based fetch regardless of `preload`, and Chromium's media
  // pipeline never recovers from that mismatch (stuck at
  // networkState=NETWORK_NO_SOURCE, MEDIA_ERR_SRC_NOT_SUPPORTED). A plain
  // fetch() never sends a Range header, so it just gets the 200 the server
  // was always going to send - fetch it ourselves and hand the video
  // element a local blob: URL instead.
  const [blobUrl, setBlobUrl] = useState(() => videoBlobCache.get(url) ?? null);

  useEffect(() => {
    if (videoBlobCache.has(url)) {
      setBlobUrl(videoBlobCache.get(url));
      return;
    }
    let disposed = false;
    void fetch(url)
      .then((response) => {
        if (!response.ok) throw new Error(`Video fetch failed: ${response.status}`);
        return response.blob();
      })
      .then((blob) => {
        if (disposed) return;
        const objectUrl = URL.createObjectURL(blob);
        videoBlobCache.set(url, objectUrl);
        setBlobUrl(objectUrl);
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
    };
  }, [url]);

  return (
    <div className="v3-tweet-embed__media-frame" onClick={(event) => event.stopPropagation()}>
      <video
        className="v3-tweet-embed__video"
        src={blobUrl ?? undefined}
        poster={poster}
        controls
        playsInline
        loop={loop}
      />
    </div>
  );
}

function TweetMedia({ media }) {
  const video = media.find((item) => item.type === "video" || item.type === "gif");
  if (video) {
    return <TweetVideo url={video.url} poster={video.thumbnailUrl} loop={video.type === "gif"} />;
  }
  const photos = media.filter((item) => item.type === "photo").slice(0, 4);
  if (photos.length === 0) return null;
  return (
    <div
      className={`v3-tweet-embed__photos v3-tweet-embed__photos--${photos.length}`}
      onClick={(event) => event.stopPropagation()}
    >
      {photos.map((photo, index) => (
        <img
          key={index}
          src={photo.url}
          alt=""
          loading="lazy"
          referrerPolicy="no-referrer"
          onClick={() => useLightbox.getState().show(photo.url)}
        />
      ))}
    </div>
  );
}

function TweetEmbed({ preview }) {
  const open = (event) => {
    event.preventDefault();
    void openLink(preview.url);
  };
  return (
    <div className="v3-tweet-embed">
      <button type="button" className="v3-tweet-embed__link" onClick={open}>
        <span className="v3-tweet-embed__source">
          <XIcon />
          Post from X
        </span>
        <span className="v3-tweet-embed__author">
          {preview.author.avatarUrl ? (
            <img src={preview.author.avatarUrl} alt="" referrerPolicy="no-referrer" />
          ) : (
            <span className="v3-tweet-embed__avatar-fallback">{preview.author.name.slice(0, 1).toUpperCase()}</span>
          )}
          <span className="v3-tweet-embed__author-text">
            <strong>{preview.author.name}</strong>
            <span>@{preview.author.handle}</span>
          </span>
        </span>
        {preview.text && <span className="v3-tweet-embed__text">{preview.text}</span>}
      </button>
      {preview.media.length > 0 && <TweetMedia media={preview.media} />}
    </div>
  );
}

function SiteEmbed({ preview }) {
  const open = (event) => {
    event.preventDefault();
    void openLink(preview.url);
  };
  return (
    <a
      className="v3-rich-link"
      href={preview.url}
      target="_blank"
      rel="noreferrer"
      onClick={open}
    >
      {preview.image && <img src={preview.image} alt="" referrerPolicy="no-referrer" />}
      <div>
        <strong>{preview.title}</strong>
        {preview.description && <span>{preview.description}</span>}
        <small>{preview.siteName || new URL(preview.url).hostname}</small>
      </div>
    </a>
  );
}

export function V3RichMessage({ content }) {
  const url = useMemo(() => {
    const first = content.match(URL_PART)?.[0];
    return first ? splitTrailingPunctuation(first).url : null;
  }, [content]);

  const [preview, setPreview] = useState(undefined);

  useEffect(() => {
    if (!url) return;
    let disposed = false;
    if (cache.has(url)) {
      setPreview(cache.get(url));
      return;
    }
    void supabase.functions
      .invoke("link-preview", { body: { url } })
      .then(({ data, error }) => {
        const next = !error && data?.preview ? data.preview : null;
        cache.set(url, next);
        if (!disposed) setPreview(next);
      })
      .catch(() => {
        cache.set(url, null);
        if (!disposed) setPreview(null);
      });
    return () => {
      disposed = true;
    };
  }, [url]);

  return (
    <>
      {content && (
        <motion.p
          className="message-template__body"
          initial={{ opacity: 0, x: -30, scale: 0.92 }}
          animate={{ opacity: [0, 1, 1], x: [-30, 3, 0], scale: [0.92, 1.01, 1] }}
          transition={{ duration: 0.36, delay: 0.215, times: [0, 0.7, 1], ease: [0.22, 0.78, 0.3, 1] }}
        >
          <LinkText content={content} hidden={preview ? url : null} />
        </motion.p>
      )}
      {preview?.kind === "tweet" && <TweetEmbed preview={preview} />}
      {preview?.kind === "site" && <SiteEmbed preview={preview} />}
    </>
  );
}
