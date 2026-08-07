import { useEffect, useState } from "react";
import { decorationUrl } from "@/lib/avatar-decorations";
import { DecoratedText } from "@/components/decorated-text";
import { nameColorClass } from "@/lib/name-colors";
import { useChat } from "@/stores/chat";
import { V3MediaAttachment } from "./V3MediaAttachment";
import { V3MessageActions } from "./V3MessageActions";
import { V3ReplyPreview } from "./V3ReplyPreview";
import { V3RichMessage } from "./V3RichMessage";

export function MessageTemplate({ name, avatar, avatarDecoration, nameDecoration, nameColor, nameFont, nameWeight, message, timestamp, isSelf, showMeta = true, media, sourceMessage, isDeleted = false, isEdited = false, decorationActive = false, onDecorationHoverChange, replyPreview = null }) {
  const className = ["message-template", isSelf && "message-template--self", !showMeta && "message-template--grouped", replyPreview && "message-template--has-reply"].filter(Boolean).join(" ");
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
          <span className="message-template__reply-hook" aria-hidden="true" />
        </>
      )}
      <span className="message-template__avatar" aria-hidden="true">
        {showMeta && <span className="message-template__avatar-photo">
          {avatar ? <img src={avatar} alt="" /> : <span className="message-template__fallback">{name.slice(0, 1).toUpperCase()}</span>}
        </span>}
        {showMeta && decoration && <img className="message-template__decoration" src={decoration} alt="" />}
      </span>
      <div className="message-template__content">
        {showMeta && <header className="message-template__header">{isSelf ? <>{timeNode}{nameNode}</> : <>{nameNode}{timeNode}</>}</header>}
        {editing ? (
          <>
            <textarea className="v3-inline-editor" autoFocus value={draft} aria-label="Edit message" onChange={(event) => setDraft(event.target.value)} onKeyDown={(event) => { if (event.key === "Escape") { event.preventDefault(); setEditing(false); } if (event.key === "Enter" && !event.shiftKey) { event.preventDefault(); if (!saving) void saveEdit(); } }} />
            <p className="v3-inline-editor-hint"><kbd>esc</kbd> to cancel &middot; <kbd>enter</kbd> to save</p>
          </>
        ) : (
          <>
            {isDeleted ? <span className="message-template__state">(deleted)</span> : <>
              {message && <V3RichMessage content={message} alignEnd={isSelf} />}
              {media && <V3MediaAttachment message={media} alignEnd={isSelf} />}
              {isEdited && <span className="message-template__state">(edited)</span>}
            </>}
          </>
        )}
        {sourceMessage && !sourceMessage.deleted_at && !editing && <V3MessageActions message={sourceMessage} onEdit={() => setEditing(true)} />}
      </div>
    </article>
  );
}
