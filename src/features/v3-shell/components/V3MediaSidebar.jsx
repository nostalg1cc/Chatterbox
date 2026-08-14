import { useEffect, useMemo, useState } from "react";
import { supabase } from "@/lib/supabase";
import { remoteMediaUrl } from "@/lib/media";
import { extractFirstUrl, resolveLinkPreview } from "@/lib/link-preview";
import { useChat } from "@/stores/chat";
import { Play } from "lucide-react";

// Tweet-status links are the only message-content links worth resolving here
// - generic site-preview cards don't carry a gallery of media the way a
// tweet's photos/video do, so they're not "media" in the sense this sidebar
// means.
const TWEET_STATUS_URL = /https?:\/\/(?:www\.)?(?:twitter|x)\.com\/[^\s<]+\/status\/[^\s<]+/i;

function afterPaint() {
  return new Promise((resolve) => requestAnimationFrame(resolve));
}

// Sidebar entries can point at messages far outside the currently-loaded
// page window (this sidebar's own query pulls full conversation history,
// while the chat pane only keeps the most recent page loaded) - so the
// target element may not exist in the DOM yet. Page in older messages
// until it shows up (or history runs out) instead of silently doing
// nothing, which is what a plain scrollIntoView did before.
async function jumpToMessage(messageId, conversationId) {
  let element = document.getElementById("message-" + messageId);
  while (!element && useChat.getState().hasMore[conversationId]) {
    await useChat.getState().loadOlder(conversationId);
    await afterPaint();
    element = document.getElementById("message-" + messageId);
  }
  element?.scrollIntoView({ behavior: "smooth", block: "center" });
}

export function V3MediaSidebar() {
  const activeId = useChat((state) => state.activeId);
  const liveMessages = useChat((state) => (activeId ? state.messages[activeId] : null));
  const [fetched, setFetched] = useState([]);
  const [tweetPreviews, setTweetPreviews] = useState({});

  useEffect(() => {
    if (!activeId) {
      setFetched([]);
      return undefined;
    }
    let cancelled = false;
    setFetched([]);
    setTweetPreviews({});
    void supabase
      .from("messages")
      .select("*")
      .eq("conversation_id", activeId)
      .is("deleted_at", null)
      .or("media_kind.not.is.null,content.ilike.%/status/%")
      .order("created_at", { ascending: false })
      .then(({ data }) => {
        if (!cancelled) setFetched(data ?? []);
      });
    return () => {
      cancelled = true;
    };
  }, [activeId]);

  const candidateMessages = useMemo(() => {
    const live = (liveMessages ?? []).filter(
      (message) => !message.deleted_at && (message.media_kind || TWEET_STATUS_URL.test(message.content ?? "")),
    );
    const byId = new Map();
    for (const message of [...fetched, ...live]) byId.set(message.id, message);
    return [...byId.values()];
  }, [fetched, liveMessages]);

  useEffect(() => {
    let cancelled = false;
    for (const message of candidateMessages) {
      if (message.media_kind || message.id in tweetPreviews) continue;
      const url = extractFirstUrl(message.content ?? "");
      if (!url || !TWEET_STATUS_URL.test(url)) continue;
      void resolveLinkPreview(url).then((preview) => {
        if (!cancelled) setTweetPreviews((prev) => ({ ...prev, [message.id]: preview }));
      });
    }
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [candidateMessages]);

  // One entry per message, not per photo - a tweet with a 4-photo grid is
  // one thing you scrolled past in chat, so it should jump back to one
  // place, with all its photos shown together the same way the tweet embed
  // itself renders them (see V3RichMessage's TweetMedia).
  const mediaItems = useMemo(() => {
    const items = [];
    for (const message of candidateMessages) {
      if (message.media_kind) {
        const url = remoteMediaUrl(message.media_path);
        if (!url) continue;
        items.push({
          id: message.id,
          messageId: message.id,
          createdAt: message.created_at,
          kind: message.media_kind,
          images: message.media_kind === "image" ? [url] : [],
          videoUrl: message.media_kind === "video" ? url : null,
        });
        continue;
      }
      const preview = tweetPreviews[message.id];
      if (preview?.kind !== "tweet" || preview.media.length === 0) continue;
      const video = preview.media.find((item) => item.type === "video" || item.type === "gif");
      if (video) {
        items.push({ id: message.id + ":video", messageId: message.id, createdAt: message.created_at, kind: "video", images: [], videoUrl: video.url });
      } else {
        const photos = preview.media.filter((item) => item.type === "photo").map((item) => item.url);
        if (photos.length === 0) continue;
        items.push({ id: message.id + ":photos", messageId: message.id, createdAt: message.created_at, kind: photos.length > 1 ? "gallery" : "image", images: photos, videoUrl: null });
      }
    }
    return items.sort((a, b) => (a.createdAt < b.createdAt ? 1 : -1));
  }, [candidateMessages, tweetPreviews]);

  return (
    <aside className="v3-media-sidebar" aria-label="Shared media">
      <div className="v3-media-sidebar__scroll">
        <div className="v3-media-sidebar__inner">
          {mediaItems.length === 0 && <p className="v3-media-sidebar__empty">No media shared yet.</p>}
          <div className="v3-media-sidebar__list">
            {mediaItems.map((item) => (
              <button
                key={item.id}
                type="button"
                className="v3-media-sidebar__item"
                onClick={() => void jumpToMessage(item.messageId, activeId)}
              >
                {item.kind === "video" && (
                  <span className="v3-media-sidebar__thumb">
                    <video src={item.videoUrl} muted preload="metadata" />
                    <span className="v3-media-sidebar__badge" aria-hidden="true"><Play /></span>
                  </span>
                )}
                {item.kind === "image" && (
                  <span className="v3-media-sidebar__thumb">
                    <img src={item.images[0]} alt="" loading="lazy" referrerPolicy="no-referrer" />
                  </span>
                )}
                {item.kind === "gallery" && (
                  <span className={"v3-media-sidebar__gallery v3-media-sidebar__gallery--" + Math.min(item.images.length, 4)}>
                    {item.images.slice(0, 4).map((src, index) => (
                      <img key={index} src={src} alt="" loading="lazy" referrerPolicy="no-referrer" />
                    ))}
                  </span>
                )}
              </button>
            ))}
          </div>
        </div>
      </div>
    </aside>
  );
}
