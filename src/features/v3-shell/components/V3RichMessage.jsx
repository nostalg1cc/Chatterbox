import { useEffect, useMemo, useState } from "react";
import { motion } from "framer-motion";
import { Play } from "lucide-react";
import { isTauri } from "@/lib/tauri";
import { openUrl } from "@tauri-apps/plugin-opener";
import { extractFirstUrl, cachedLinkPreview, resolveLinkPreview, tweetVideoProxyUrl } from "@/lib/link-preview";
import { useAlerts } from "@/stores/alerts";
import { useLightbox } from "@/stores/lightbox";

const URL_PART = /(https?:\/\/[^\s<]+)/gi;
const URL_ONLY = /^https?:\/\/[^\s<]+$/i;

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
  // video.twimg.com 403s a plain browser fetch/<video src> (it checks for
  // a Referer scripts are forbidden from setting) - tweetVideoProxyUrl
  // routes through a Supabase Edge Function that fetches it server-side
  // with that header instead, forwarding Range requests so native
  // scrubbing still works. See tweet-video-proxy for the actual proxy.
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    setFailed(false);
  }, [url]);

  if (failed) {
    return (
      <div className="v3-tweet-embed__media-frame v3-tweet-embed__media-frame--unavailable" onClick={(event) => event.stopPropagation()}>
        Video unavailable
      </div>
    );
  }

  return (
    <div className="v3-tweet-embed__media-frame" onClick={(event) => event.stopPropagation()}>
      <video
        className="v3-tweet-embed__video"
        src={tweetVideoProxyUrl(url)}
        poster={poster}
        controls
        playsInline
        preload="metadata"
        loop={loop}
        onError={() => setFailed(true)}
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

// YouTube officially supports cross-site <iframe src=".../embed/ID">
// embedding - no CORS/Referer fight, no proxy needed, unlike the tweet
// video CDN. Still click-to-load rather than embedding the iframe
// unconditionally, so scrolling through chat history doesn't load
// YouTube's player (and its own tracking/scripts) for every video someone
// ever linked - only the ones actually watched.
function YouTubeEmbed({ preview }) {
  const [playing, setPlaying] = useState(false);

  if (playing) {
    return (
      <div className="v3-youtube-embed v3-youtube-embed--playing" onClick={(event) => event.stopPropagation()}>
        <iframe
          className="v3-youtube-embed__frame"
          src={`https://www.youtube-nocookie.com/embed/${preview.videoId}?autoplay=1`}
          title={preview.title}
          allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture"
          allowFullScreen
        />
      </div>
    );
  }

  return (
    <button
      type="button"
      className="v3-youtube-embed"
      onClick={(event) => {
        event.stopPropagation();
        setPlaying(true);
      }}
    >
      <span className="v3-youtube-embed__thumb">
        <img src={preview.thumbnail} alt="" referrerPolicy="no-referrer" />
        <span className="v3-youtube-embed__play" aria-hidden="true"><Play /></span>
      </span>
      <span className="v3-youtube-embed__meta">
        <strong>{preview.title}</strong>
        {preview.authorName && <span>{preview.authorName}</span>}
      </span>
    </button>
  );
}

export function V3RichMessage({ content }) {
  const url = useMemo(() => extractFirstUrl(content), [content]);

  const [preview, setPreview] = useState(undefined);

  useEffect(() => {
    if (!url) return;
    let disposed = false;
    const cached = cachedLinkPreview(url);
    if (cached !== undefined) {
      setPreview(cached);
      return;
    }
    void resolveLinkPreview(url).then((next) => {
      if (!disposed) setPreview(next);
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
      {preview?.kind === "youtube" && <YouTubeEmbed preview={preview} />}
      {preview?.kind === "site" && <SiteEmbed preview={preview} />}
    </>
  );
}
