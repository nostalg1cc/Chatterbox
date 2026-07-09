import { useEffect } from "react";
import { UserAvatar } from "@/components/user-avatar";
import { Composer } from "./composer";
import { MessageList } from "./message-list";
import { useAuth } from "@/stores/auth";
import { useChat } from "@/stores/chat";
import { useIsOnline } from "@/stores/presence";
import { useProfiles } from "@/stores/profiles";

export function ChatView({ conversationId }: { conversationId: string }) {
  const myId = useAuth((s) => s.userId) ?? "";
  const conv = useChat((s) => s.conversations.find((c) => c.id === conversationId));
  const friendId = conv
    ? conv.user1_id === myId
      ? conv.user2_id
      : conv.user1_id
    : undefined;
  const friend = useProfiles((s) => (friendId ? s.byId[friendId] : undefined));
  const online = useIsOnline(friendId);
  const typingUserId = useChat((s) => s.typing[conversationId]);
  const isTyping = Boolean(typingUserId && typingUserId === friendId);

  useEffect(() => useChat.getState().joinTyping(conversationId), [conversationId]);

  useEffect(() => {
    const onFocus = () => useChat.getState().markRead(conversationId);
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [conversationId]);

  return (
    <div className="flex h-full min-h-0 flex-col">
      <header className="flex h-12 shrink-0 items-center gap-2.5 border-b px-4">
        <UserAvatar profile={friend} online={online} />
        <div className="min-w-0 leading-tight">
          <p className="truncate text-sm font-medium">{friend?.display_name ?? "…"}</p>
          <p className="truncate text-xs text-muted-foreground">
            {isTyping ? "typing…" : online ? "Online" : "Offline"}
          </p>
        </div>
      </header>

      <MessageList conversationId={conversationId} />

      <div className="h-5 shrink-0 px-4 text-xs text-muted-foreground">
        {isTyping && <span>{friend?.display_name ?? "Friend"} is typing…</span>}
      </div>

      <Composer
        conversationId={conversationId}
        placeholder={friend ? `Message @${friend.username}` : "Message"}
      />
    </div>
  );
}
