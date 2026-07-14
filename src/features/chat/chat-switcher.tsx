import { ChevronDownIcon, HeadphonesIcon, ImagesIcon, MessageSquareIcon, UsersIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { ScrollArea } from "@/components/ui/scroll-area";
import { UserAvatar } from "@/components/user-avatar";
import { DecoratedText } from "@/components/decorated-text";
import { AddFriendDialog } from "@/features/friends/add-friend-dialog";
import { relativeTime } from "@/lib/format";
import { nameColorClass } from "@/lib/name-colors";
import { cn } from "@/lib/utils";
import { useAuth } from "@/stores/auth";
import { useChat } from "@/stores/chat";
import { useFriends } from "@/stores/friends";
import { useIsOnline } from "@/stores/presence";
import { useProfiles } from "@/stores/profiles";
import { useVoice } from "@/stores/voice";

export function ChatSwitcher({ conversationId }: { conversationId: string }) {
  const myId = useAuth((state) => state.userId) ?? "";
  const conversations = useChat((state) => state.conversations);
  const pendingIncoming = useFriends((state) =>
    state.friendships.filter(
      (friendship) => friendship.status === "pending" && friendship.addressee_id === myId
    ).length
  );
  const activeConversation = conversations.find((conversation) => conversation.id === conversationId);
  const friendId = activeConversation
    ? activeConversation.user1_id === myId
      ? activeConversation.user2_id
      : activeConversation.user1_id
    : undefined;
  const friend = useProfiles((state) => (friendId ? state.byId[friendId] : undefined));
  const online = useIsOnline(friendId);
  const channel = useChat((state) => state.channel);
  const activeVoiceId = useVoice((state) => state.activeConversationId);
  const isJoined = activeVoiceId === conversationId;

  return (
    <Popover>
      <PopoverTrigger asChild>
        <button
          type="button"
          className="dock-profile app-control flex h-9 w-[164px] shrink-0 items-center gap-2 px-2 text-left transition-colors hover:bg-white/[0.06] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/60"
          aria-label="Open chats and channels"
        >
          <UserAvatar profile={friend} online={online} size="sm" animated />
          <span className="min-w-0 flex-1 leading-[1.05]">
            <span className={cn("block truncate text-sm font-medium", nameColorClass(friend?.name_color))}>
              <DecoratedText effect={friend?.name_decoration as never} font={friend?.name_font} weight={friend?.name_weight} active>{friend?.display_name ?? "..."}</DecoratedText>
            </span>
            <span className="block truncate text-xs text-muted-foreground">
              {channel === "media" ? "Media channel" : online ? "Online" : "Offline"}
            </span>
          </span>
          <ChevronDownIcon className="mr-1 size-3.5 shrink-0 text-muted-foreground" />
        </button>
      </PopoverTrigger>
      <PopoverContent align="start" sideOffset={8} className="app-surface w-[310px] gap-2 p-2.5">
        <div className="flex flex-col gap-0.5 px-1 pt-0.5">
          <Button
            variant="ghost"
            size="sm"
            className="w-full justify-start text-muted-foreground"
            onClick={() => useChat.getState().setView("friends")}
          >
            <UsersIcon />
            Friends
            {pendingIncoming > 0 && <span className="ml-auto text-xs tabular-nums text-foreground">{pendingIncoming}</span>}
          </Button>
          <AddFriendDialog fullWidth />
        </div>

        <div className="px-2 pt-1 text-[10px] font-semibold tracking-[0.11em] text-muted-foreground/75 uppercase">
          Direct messages
        </div>
        <ScrollArea className="max-h-64">
          <div className="space-y-0.5 p-1">
            {conversations.map((conversation) => (
              <ConversationChoice key={conversation.id} conversationId={conversation.id} />
            ))}
            {conversations.length === 0 && (
              <p className="px-2 py-5 text-center text-xs text-muted-foreground">No chats yet. Add a friend to begin.</p>
            )}
          </div>
        </ScrollArea>

        <div className="border-t border-white/[0.11] px-1 pt-2">
          <p className="px-1 pb-1 text-[10px] font-semibold tracking-[0.11em] text-muted-foreground/75 uppercase">This chat</p>
          <div className="flex flex-col gap-0.5">
            <Button variant={channel === "chat" ? "secondary" : "ghost"} size="sm" className="w-full justify-start" onClick={() => useChat.getState().openConversationChannel(conversationId, "chat")}>
              <MessageSquareIcon />
              Chat
            </Button>
            <Button variant={channel === "media" ? "secondary" : "ghost"} size="sm" className="w-full justify-start" onClick={() => useChat.getState().openConversationChannel(conversationId, "media")}>
              <ImagesIcon />
              Media
            </Button>
            <Button variant={isJoined ? "secondary" : "ghost"} size="sm" className="w-full justify-start" onClick={() => void useVoice.getState().join(conversationId)}>
              <HeadphonesIcon />
              {isJoined ? "Voice connected" : "Join voice"}
            </Button>
          </div>
        </div>
      </PopoverContent>
    </Popover>
  );
}

function ConversationChoice({ conversationId }: { conversationId: string }) {
  const myId = useAuth((state) => state.userId);
  const conversation = useChat((state) => state.conversations.find((entry) => entry.id === conversationId));
  const active = useChat((state) => state.activeId === conversationId && state.view === "chat");
  const overview = useChat((state) => state.overviews[conversationId]);
  const unread = useChat((state) => state.unread[conversationId] ?? 0);
  const friendId = conversation && myId
    ? conversation.user1_id === myId ? conversation.user2_id : conversation.user1_id
    : undefined;
  const friend = useProfiles((state) => (friendId ? state.byId[friendId] : undefined));
  const online = useIsOnline(friendId);

  if (!conversation) return null;
  return (
    <button
      type="button"
      className={cn(
        "app-control flex w-full items-center gap-2.5 px-2.5 py-2.5 text-left transition-colors hover:bg-white/[0.07] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/60",
        active && "bg-white/[0.10]"
      )}
      onClick={() => useChat.getState().openConversation(conversationId)}
    >
      <UserAvatar profile={friend} online={online} size="sm" />
      <span className="min-w-0 flex-1">
        <span className={cn("block truncate text-sm font-medium", nameColorClass(friend?.name_color))}><DecoratedText effect={friend?.name_decoration as never} font={friend?.name_font} weight={friend?.name_weight} active>{friend?.display_name ?? "..."}</DecoratedText></span>
        <span className="block truncate text-[11px] text-muted-foreground">{overview?.content || "Say hi."}</span>
      </span>
      <span className="flex shrink-0 flex-col items-end gap-1">
        {overview?.at && <span className="text-[10px] text-muted-foreground/70">{relativeTime(overview.at)}</span>}
        {unread > 0 && <span className="min-w-4 rounded-full bg-foreground px-1 text-center text-[9px] leading-4 text-background">{unread > 99 ? "99+" : unread}</span>}
      </span>
    </button>
  );
}