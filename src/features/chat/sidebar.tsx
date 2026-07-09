import { UsersIcon } from "lucide-react";
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
import { SettingsDialog } from "@/features/settings/settings-dialog";
import { relativeTime } from "@/lib/format";
import type { Conversation } from "@/lib/types";
import { cn } from "@/lib/utils";
import { useAuth } from "@/stores/auth";
import { useChat } from "@/stores/chat";
import { useFriends } from "@/stores/friends";
import { useIsOnline, usePresence } from "@/stores/presence";
import { useProfiles } from "@/stores/profiles";

export function Sidebar() {
  const conversations = useChat((s) => s.conversations);
  const view = useChat((s) => s.view);
  const pendingIncoming = useFriends(
    (s) =>
      s.friendships.filter(
        (f) => f.status === "pending" && f.addressee_id === useAuth.getState().userId
      ).length
  );

  return (
    <aside className="flex w-72 shrink-0 flex-col">
      <div className="flex items-center gap-1 px-3 pt-2 pb-2">
        <Button
          variant="ghost"
          size="sm"
          className={cn(
            "flex-1 justify-start text-muted-foreground",
            view === "friends" && "bg-muted/60 text-foreground"
          )}
          onClick={() => useChat.getState().setView("friends")}
        >
          <UsersIcon />
          Friends
          {pendingIncoming > 0 && (
            <Badge className="ml-auto h-4 min-w-4 rounded-full px-1 text-[10px]">
              {pendingIncoming}
            </Badge>
          )}
        </Button>
        <AddFriendDialog />
      </div>

      <ScrollArea className="min-h-0 flex-1">
        <nav className="flex flex-col gap-0.5 px-2 pb-2">
          {conversations.map((conv) => (
            <ConversationItem key={conv.id} conv={conv} />
          ))}
          {conversations.length === 0 && (
            <p className="px-2 py-6 text-center text-xs text-muted-foreground/70">
              No conversations yet.
              <br />
              Friends you add will show up here.
            </p>
          )}
        </nav>
      </ScrollArea>

      <SidebarFooter />
    </aside>
  );
}

function ConversationItem({ conv }: { conv: Conversation }) {
  const myId = useAuth((s) => s.userId);
  const friendId = conv.user1_id === myId ? conv.user2_id : conv.user1_id;
  const friend = useProfiles((s) => s.byId[friendId]);
  const online = useIsOnline(friendId);
  const active = useChat((s) => s.activeId === conv.id && s.view === "chat");
  const overview = useChat((s) => s.overviews[conv.id]);
  const unread = useChat((s) => s.unread[conv.id] ?? 0);
  const friendship = useFriends((s) =>
    s.friendships.find(
      (f) =>
        f.status === "accepted" &&
        ((f.requester_id === friendId && f.addressee_id === myId) ||
          (f.requester_id === myId && f.addressee_id === friendId))
    )
  );

  const preview = overview?.content
    ? `${overview.senderId === myId ? "You: " : ""}${
        overview.deleted ? "Message deleted" : overview.content
      }`
    : "Say hi.";

  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>
        <button
          className={cn(
            "flex w-full items-center gap-2.5 rounded-md px-2 py-2 text-left transition-colors outline-none",
            "hover:bg-muted/50 focus-visible:ring-2 focus-visible:ring-ring/50",
            active && "bg-muted/70"
          )}
          onClick={() => useChat.getState().openConversation(conv.id)}
        >
          <UserAvatar profile={friend} online={online} />
          <span className="min-w-0 flex-1">
            <span className="flex items-baseline justify-between gap-2">
              <span className="truncate text-sm font-medium">
                {friend?.display_name ?? "…"}
              </span>
              {overview?.at && (
                <span className="shrink-0 text-[10px] text-muted-foreground/70">
                  {relativeTime(overview.at)}
                </span>
              )}
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
              {unread > 0 && (
                <Badge className="h-4 min-w-4 shrink-0 rounded-full px-1 text-[10px]">
                  {unread > 99 ? "99+" : unread}
                </Badge>
              )}
            </span>
          </span>
        </button>
      </ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItem onClick={() => useChat.getState().markRead(conv.id)}>
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
  );
}

function SidebarFooter() {
  const profile = useAuth((s) => s.profile);
  const myId = useAuth((s) => s.userId);
  const online = usePresence((s) => (myId ? Boolean(s.online[myId]) : false));

  return (
    <div className="flex items-center gap-2 border-t px-3 py-2.5">
      <UserAvatar profile={profile} online={online} />
      <div className="min-w-0 flex-1 leading-tight">
        <p className="truncate text-sm font-medium">{profile?.display_name ?? "…"}</p>
        <p className="truncate text-xs text-muted-foreground">
          @{profile?.username ?? ""}
        </p>
      </div>
      <SettingsDialog />
    </div>
  );
}
