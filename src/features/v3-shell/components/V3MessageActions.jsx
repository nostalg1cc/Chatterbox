import { useMemo, useState } from "react";
import { Pencil, Reply, SmilePlus, Trash2 } from "lucide-react";
import { useAuth } from "@/stores/auth";
import { useChat } from "@/stores/chat";

const QUICK_REACTIONS = ["❤", "😂", "👍", "👀", "🔥", "🎉"];
const EMPTY_REACTIONS = [];

export function V3MessageActions({ message, onEdit }) {
  const [pickerOpen, setPickerOpen] = useState(false);
  const userId = useAuth((state) => state.userId);
  const reactions = useChat((state) => state.reactions[message.id] ?? EMPTY_REACTIONS);
  const groups = useMemo(() => Object.values(reactions.reduce((result, reaction) => {
    const existing = result[reaction.emoji] ?? { emoji: reaction.emoji, count: 0, mine: false };
    existing.count += 1;
    existing.mine ||= reaction.user_id === userId;
    result[reaction.emoji] = existing;
    return result;
  }, {})), [reactions, userId]);
  const canEdit = message.sender_id === userId && !message.deleted_at;
  const react = (emoji) => { void useChat.getState().toggleReaction(message, emoji); setPickerOpen(false); };

  return <>
    <div className="v3-message-actions" role="toolbar" aria-label="Message actions">
      <button type="button" aria-label="Add reaction" onClick={() => setPickerOpen((value) => !value)}><SmilePlus /></button>
      <button type="button" aria-label="Reply" onClick={() => useChat.getState().setReplyTo(message)}><Reply /></button>
      {canEdit && <button type="button" aria-label="Edit" onClick={onEdit}><Pencil /></button>}
      {canEdit && <button type="button" aria-label="Delete" onClick={() => void useChat.getState().deleteMessage(message.id)}><Trash2 /></button>}
      {pickerOpen && <div className="v3-reaction-picker" role="menu" aria-label="Choose a reaction">{QUICK_REACTIONS.map((emoji) => <button key={emoji} type="button" role="menuitem" onClick={() => react(emoji)}>{emoji}</button>)}</div>}
    </div>
    {groups.length > 0 && <div className="v3-reaction-chips" aria-label="Message reactions">{groups.map((group) => <button key={group.emoji} type="button" className={group.mine ? "is-mine" : ""} onClick={() => react(group.emoji)}><span>{group.emoji}</span><span>{group.count}</span></button>)}</div>}
  </>;
}
