import { Play, Reply as ReplyIcon } from "lucide-react";
import { nameColorClass } from "@/lib/name-colors";

const CLOUDINARY_BASE = "https://res.cloudinary.com/lnkoms9m";

function replyThumbnailUrl(path) {
  const match = /^cloudinary:(image|video):([0-9a-f-]{36}_[0-9a-f-]{36})$/i.exec(path ?? "");
  if (!match) return null;
  const [, mediaKind, id] = match;
  return mediaKind === "image"
    ? `${CLOUDINARY_BASE}/image/upload/c_fill,w_64,h_64,g_auto/f_webp/q_auto:eco/dislight/chat-media/${id}.webp`
    : `${CLOUDINARY_BASE}/video/upload/so_0,c_fill,w_64,h_64,g_auto/q_auto:eco/dislight/chat-media/${id}.jpg`;
}

export function V3ReplyPreview({ target, authorName, authorAvatar, authorNameColor, onJump }) {
  if (!target) {
    return (
      <span className="message-template__reply-row message-template__reply-row--unavailable">
        <ReplyIcon className="message-template__reply-icon" aria-hidden="true" />
        <span className="message-template__reply-copy">Original message unavailable</span>
      </span>
    );
  }

  const caption = target.deleted_at ? "Message deleted" : target.content || null;
  const thumbnail = !target.deleted_at ? replyThumbnailUrl(target.media_path) : null;
  const excerpt = caption ?? (thumbnail
    ? null
    : target.media_kind === "image" ? "Photo" : target.media_kind === "video" ? "Video" : "Message");

  return (
    <button type="button" className="message-template__reply-row" onClick={onJump} aria-label="Jump to replied message">
      <span className="message-template__reply-avatar" aria-hidden="true">
        {authorAvatar ? <img src={authorAvatar} alt="" /> : <ReplyIcon className="message-template__reply-icon" />}
      </span>
      <span className={"message-template__reply-name " + nameColorClass(authorNameColor)}>{authorName}</span>
      {thumbnail && (
        <span className="message-template__reply-thumb">
          <img src={thumbnail} alt="" loading="lazy" />
          {target.media_kind === "video" && <Play className="message-template__reply-thumb-play" aria-hidden="true" />}
        </span>
      )}
      {excerpt && <span className="message-template__reply-copy">{excerpt}</span>}
    </button>
  );
}
