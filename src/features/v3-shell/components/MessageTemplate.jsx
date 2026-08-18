import { useEffect, useState } from "react";
import { motion } from "framer-motion";
import { decorationUrl } from "@/lib/avatar-decorations";
import { DecoratedText } from "@/components/decorated-text";
import { nameColorClass } from "@/lib/name-colors";
import { useChat } from "@/stores/chat";
import { V3MediaAttachment } from "./V3MediaAttachment";
import { V3MessageActions } from "./V3MessageActions";
import { V3ReplyPreview } from "./V3ReplyPreview";
import { V3RichMessage } from "./V3RichMessage";

// Same curve/timing as the old message-avatar-in / message-copy-in CSS
// keyframes, just driven by framer-motion instead.
const AVATAR_ANIMATE = { opacity: [0, 1, 1, 1], x: [-56, 7, -1, 0], scale: [0.62, 1.03, 0.996, 1] };
const AVATAR_TRANSITION = { duration: 0.38, times: [0, 0.64, 0.83, 1], ease: [0.22, 0.72, 0.3, 1] };
const HEADER_ANIMATE = { opacity: [0, 1, 1], x: [-30, 3, 0], scale: [0.92, 1.01, 1] };
const HEADER_TRANSITION = { duration: 0.33, delay: 0.115, times: [0, 0.7, 1], ease: [0.22, 0.78, 0.3, 1] };

export function MessageTemplate({ name, avatar, avatarDecoration, nameDecoration, nameColor, nameFont, nameWeight, message, timestamp, showMeta = true, media, sourceMessage, isDeleted = false, deletedCount = 1, isEdited = false, decorationActive = false, onDecorationHoverChange, replyPreview = null }) {
  const className = ["message-template", !showMeta && "message-template--grouped", replyPreview && "message-template--has-reply"].filter(Boolean).join(" ");
  const [hovered, setHovered] = useState(false);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(sourceMessage?.content ?? "");
  const [saving, setSaving] = useState(false);
  const decoration = decorationUrl(avatarDecoration, decorationActive || hovered);

  useEffect(() => {
    if (!editing) setDraft(sourceMessage?.content ?? "");
  }, [editing, sourceMessage?.content]);

  async function saveEdit() {
    const next = draft.trim();
    if (!sourceMessage || !next || next === sourceMessage.content) {
      setEditing(false);
      return;
    }
    setSaving(true);
    await useChat.getState().editMessage(sourceMessage.id, next);
    setSaving(false);
    setEditing(false);
  }

  const nameNode = <DecoratedText effect={nameDecoration} font={nameFont} weight={nameWeight} className={"message-template__name " + nameColorClass(nameColor)}>{name}</DecoratedText>;
  const timeNode = <time className="message-template__timestamp">{timestamp}</time>;

  return (
    <article id={sourceMessage ? "message-" + sourceMessage.id : undefined} className={className} aria-label={"Message from " + name + " at " + timestamp} onPointerEnter={() => { setHovered(true); onDecorationHoverChange?.(true); }} onPointerLeave={() => { setHovered(false); onDecorationHoverChange?.(false); }}>
      {replyPreview && (
        <>
          <V3ReplyPreview target={replyPreview.target} authorName={replyPreview.authorName} authorAvatar={replyPreview.authorAvatar} authorNameColor={replyPreview.authorNameColor} onJump={replyPreview.onJump} />
          <span className="message-template__reply-hook" aria-hidden="true">
            <svg viewBox="0 0 48 20" fill="none">
              <path d="M44 5 H27 Q19 5 19 13 V20" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
            </svg>
          </span>
        </>
      )}
      <motion.span
        className="message-template__avatar"
        aria-hidden="true"
        initial={showMeta ? { opacity: 0, x: -56, scale: 0.62 } : false}
        animate={AVATAR_ANIMATE}
        transition={AVATAR_TRANSITION}
      >
        {showMeta && <span className="message-template__avatar-photo">
          {avatar ? <img src={avatar} alt="" /> : <span className="message-template__fallback">{name.slice(0, 1).toUpperCase()}</span>}
        </span>}
        {showMeta && decoration && <img className="message-template__decoration" src={decoration} alt="" />}
        {!showMeta && <time className="message-template__hover-timestamp" aria-hidden="true">{timestamp}</time>}
      </motion.span>
      <div className="message-template__content">
        {showMeta && (
          <motion.header
            className="message-template__header"
            initial={{ opacity: 0, x: -30, scale: 0.92 }}
            animate={HEADER_ANIMATE}
            transition={HEADER_TRANSITION}
          >
            {nameNode}{timeNode}
          </motion.header>
        )}
        {editing ? (
          <>
            <textarea className="v3-inline-editor" autoFocus value={draft} aria-label="Edit message" onChange={(event) => setDraft(event.target.value)} onKeyDown={(event) => { if (event.key === "Escape") { event.preventDefault(); setEditing(false); } if (event.key === "Enter" && !event.shiftKey) { event.preventDefault(); if (!saving) void saveEdit(); } }} />
            <p className="v3-inline-editor-hint"><kbd>esc</kbd> to cancel &middot; <kbd>enter</kbd> to save</p>
          </>
        ) : (
          <>
            {isDeleted ? <span className="message-template__state">{deletedCount > 1 ? `(${deletedCount} messages deleted)` : "(deleted)"}</span> : <>
              {message && <V3RichMessage content={message} />}
              {media && <V3MediaAttachment message={media} />}
              {isEdited && <span className="message-template__state">(edited)</span>}
            </>}
          </>
        )}
      </div>
      {/* A direct child of <article> (not nested in .message-template__content)
          specifically so it positions relative to the whole message card's
          top edge - a reply preview renders above .message-template__content
          as its own sibling, so anchoring the tray to that div's top instead
          of the article's would land it lower than the card's true top for
          any message with a reply attached. */}
      {sourceMessage && !sourceMessage.deleted_at && !editing && <V3MessageActions message={sourceMessage} onEdit={() => setEditing(true)} />}
    </article>
  );
}
