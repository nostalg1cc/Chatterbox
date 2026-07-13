import {
  ChevronDownIcon,
  HeadphonesIcon,
  ImagesIcon,
  UsersIcon,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { ScrollArea } from "@/components/ui/scroll-area";
import { UserAvatar } from "@/components/user-avatar";
import { AddFriendDialog } from "@/features/friends/add-friend-dialog";
import { relativeTime } from "@/lib/format";
import { nameColorClass } from "@/lib/name-colors";
import type { Conversation } from "@/lib/types";
import { cn } from "@/lib/utils";
import { useAuth } from "@/stores/auth";
import { useChat } from "@/stores/chat";
import { useFriends } from "@/stores/friends";
import { useIsOnline } from "@/stores/presence";
import { useProfiles } from "@/stores/profiles";
import { useVoice } from "@/stores/voice";
import { ScreenSharePreview } from "./screen-share-preview";
import { UserPanel } from "./user-panel";

export function Sidebar() {
  const conversations = useChat((state) => state.conversations);
  const view = useChat((state) => state.view);
  const pendingIncoming = useFriends(
    (state) =>
      state.friendships.filter(
        (friendship) =>
          friendship.status === "pending" &&
          friendship.addressee_id === useAuth.getState().userId
      ).length
  );

  return (
    <aside className="flex w-72 shrink-0 flex-col pt-8">
      <div className="flex items-center gap-1 px-3 pt-2 pb-2">
        <Button
          variant="ghost"
          size="sm"
          className={cn(
            "flex-1 justify-start text-muted-foreground",
            view === "friends" &&
              "bg-muted/85 text-foreground ring-1 ring-inset ring-white/[0.12]"
          )}
          onClick={() => useChat.getState().setView("friends")}
        >
          <UsersIcon />
          Friends
          {pendingIncoming > 0 && (
            <Badge className="ml-auto h-[17px] min-w-[17px] rounded-full px-1 text-[10px]">
              {pendingIncoming}
            </Badge>
          )}
        </Button>
        <AddFriendDialog />
      </div>

      <div className="flex items-center px-4 pt-1 pb-1.5">
        <p className="text-[10px] font-semibold tracking-[0.11em] text-muted-foreground/80 uppercase">
          Direct messages
        </p>
        <span className="ml-auto text-[10px] tabular-nums text-muted-foreground/55">
          {conversations.length}
        </span>
      </div>

      <ScrollArea className="min-h-0 flex-1">
        <nav className="flex flex-col gap-0.5 px-2 pb-2">
          {conversations.map((conversation) => (
            <ConversationItem key={conversation.id} conversation={conversation} />
          ))}
          {conversations.length === 0 && (
            <p className="px-2 py-6 text-center text-xs text-muted-foreground/80">
              No conversations yet.
              <br />
              Friends you add will show up here.
            </p>
          )}
        </nav>
      </ScrollArea>

      <ScreenSharePreview />
      <UserPanel />
    </aside>
  );
}

function ConversationItem({ conversation }: { conversation: Conversation }) {
  const myId = useAuth((state) => state.userId);
  const friendId =
    conversation.user1_id === myId ? conversation.user2_id : conversation.user1_id;
  const friend = useProfiles((state) => state.byId[friendId]);
  const online = useIsOnline(friendId);
  const active = useChat(
    (state) => state.activeId === conversation.id && state.view === "chat"
  );
  const channel = useChat((state) => state.channel);
  const overview = useChat((state) => state.overviews[conversation.id]);
  const unread = useChat((state) => state.unread[conversation.id] ?? 0);
  const voiceActive = useVoice(
    (state) => (state.participants[conversation.id]?.length ?? 0) > 0
  );
  const activeVoiceId = useVoice((state) => state.activeConversationId);
  const voiceStatus = useVoice((state) => state.status);
  const joinedHere = activeVoiceId === conversation.id;
  const friendship = useFriends((state) =>
    state.friendships.find(
      (entry) =>
        entry.status === "accepted" &&
        ((entry.requester_id === friendId && entry.addressee_id === myId) ||
          (entry.requester_id === myId && entry.addressee_id === friendId))
    )
  );

  const preview = overview?.content
    ? (overview.senderId === myId ? "You: " : "") +
      (overview.deleted ? "Message deleted" : overview.content)
    : "Say hi.";

  const openVoice = () => {
    useChat.getState().openConversationChannel(conversation.id, "chat");
    if (!joinedHere) void useVoice.getState().join(conversation.id);
  };

  return (
    <div>
      <ContextMenu>
        <ContextMenuTrigger asChild>
          <button
            className={cn(
              "flex w-full items-center gap-2.5 rounded-md border border-transparent px-2 py-2 text-left transition-colors outline-none",
              "hover:bg-muted/65 hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/60",
              active && "border-white/[0.13] bg-muted/90 ring-1 ring-inset ring-white/[0.10]"
            )}
            onClick={() => useChat.getState().openConversation(conversation.id)}
          >
            <UserAvatar profile={friend} online={online} />
            <span className="min-w-0 flex-1">
              <span className="flex items-baseline justify-between gap-2">
                <span
                  className={cn(
                    "truncate text-sm font-medium",
                    nameColorClass(friend?.name_color)
                  )}
                >
                  {friend?.display_name ?? "..."}
                </span>
                <span className="flex shrink-0 items-center gap-1.5">
                  {active && <ChevronDownIcon className="size-3 text-muted-foreground/70" />}
                  {overview?.at && (
                    <span className="text-[10px] text-muted-foreground/75">
                      {relativeTime(overview.at)}
                    </span>
                  )}
                </span>
              </span>
              <span className="flex items-center justify-between gap-2">
                <span
                  className={cn(
                    "truncate text-xs text-muted-foreground",
                    overview?.deleted && "italic",
                    unread > 0 && "font-medium text-foreground"
                  )}
                >
                  {preview}
                </span>
                {voiceActive && (
                  <span title="Voice channel active" className="shrink-0 text-emerald-400/80">
                    <HeadphonesIcon className="size-3.5" />
                  </span>
                )}
                {unread > 0 && (
                  <Badge className="h-[17px] min-w-[17px] shrink-0 rounded-full px-1 text-[10px]">
                    {unread > 99 ? "99+" : unread}
                  </Badge>
                )}
              </span>
            </span>
          </button>
        </ContextMenuTrigger>
        <ContextMenuContent>
          <ContextMenuItem onClick={() => useChat.getState().markRead(conversation.id)}>
            Mark as read
          </ContextMenuItem>
          {friendship && (
            <ContextMenuItem
              variant="destructive"
              onClick={() => void useFriends.getState().removeFriendship(friendship.id)}
            >
              Remove friend
            </ContextMenuItem>
          )}
        </ContextMenuContent>
      </ContextMenu>

      {active && (
        <div className="mt-1.5 mr-1 mb-1 ml-4 border-l border-white/[0.12] pb-1 pl-2.5">
          <p className="mb-1 px-2 text-[9px] font-semibold tracking-[0.12em] text-muted-foreground/55 uppercase">
            Channels
          </p>
          <div className="space-y-1">
            <ChannelButton
              icon={<HeadphonesIcon />}
              label={joinedHere ? voiceChannelLabel(voiceStatus) : voiceActive ? "Join voice" : "Voice"}
              active={joinedHere}
              live={voiceActive}
              onClick={openVoice}
            />
            <ChannelButton
              icon={<ImagesIcon />}
              label="Media"
              active={channel === "media"}
              onClick={() =>
                useChat.getState().openConversationChannel(conversation.id, "media")
              }
            />
          </div>
        </div>
      )}
    </div>
  );
}

function ChannelButton({
  icon,
  label,
  active,
  live = false,
  onClick,
}: {
  icon: React.ReactNode;
  label: string;
  active: boolean;
  live?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      className={cn(
        "flex h-7 w-full items-center gap-2 rounded-md border border-transparent px-2 text-left text-[11px] font-medium text-muted-foreground transition-colors",
        "hover:border-white/[0.09] hover:bg-white/[0.055] hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/60",
        active && "border-white/[0.12] bg-white/[0.09] text-foreground shadow-[inset_0_1px_0_rgb(255_255_255_/_0.06)]"
      )}
      onClick={onClick}
    >
      <span className={cn("flex size-4 shrink-0 items-center justify-center [&>svg]:size-3.5", live && "text-emerald-400/85")}>{icon}</span>
      <span className="truncate">{label}</span>
      {live && <span className="ml-auto size-1.5 rounded-full bg-emerald-400" />}
    </button>
  );
}

function voiceChannelLabel(status: ReturnType<typeof useVoice.getState>["status"]): string {
  if (status === "solo") return "Voice \u00B7 waiting";
  if (status === "connected") return "Voice \u00B7 connected";
  if (status === "reconnecting") return "Voice \u00B7 reconnecting";
  return "Voice \u00B7 connecting";
}

