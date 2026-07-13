import { CheckIcon, MessageSquareIcon, UserMinusIcon, XIcon } from "lucide-react";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { UserAvatar } from "@/components/user-avatar";
import type { Friendship } from "@/lib/types";
import { useAuth } from "@/stores/auth";
import { useChat } from "@/stores/chat";
import { useFriends } from "@/stores/friends";
import { useIsOnline } from "@/stores/presence";
import { useProfiles } from "@/stores/profiles";

export function FriendsView() {
  const myId = useAuth((s) => s.userId) ?? "";
  const friendships = useFriends((s) => s.friendships);

  const accepted = friendships.filter((f) => f.status === "accepted");
  const incoming = friendships.filter(
    (f) => f.status === "pending" && f.addressee_id === myId
  );
  const outgoing = friendships.filter(
    (f) => f.status === "pending" && f.requester_id === myId
  );

  return (
    <div className="flex h-full flex-col">
      <Tabs defaultValue="all" className="flex h-full min-h-0 flex-col gap-0">
        <div className="flex h-12 shrink-0 items-center gap-4 border-b px-4">
          <h1 className="text-sm font-semibold">Friends</h1>
          <TabsList variant="line" className="h-full">
            <TabsTrigger value="all">All</TabsTrigger>
            <TabsTrigger value="pending">
              Pending
              {incoming.length > 0 && (
                <Badge className="h-[17px] min-w-[17px] rounded-full px-1 text-[10px]">
                  {incoming.length}
                </Badge>
              )}
            </TabsTrigger>
          </TabsList>
        </div>

        <TabsContent value="all" className="min-h-0">
          <ScrollArea className="h-full">
            <div className="flex flex-col gap-0.5 p-2">
              {accepted.length === 0 && (
                <EmptyHint text="No friends yet. Send a request from the sidebar." />
              )}
              {accepted.map((f) => (
                <FriendRow key={f.id} friendship={f} myId={myId} kind="accepted" />
              ))}
            </div>
          </ScrollArea>
        </TabsContent>

        <TabsContent value="pending" className="min-h-0">
          <ScrollArea className="h-full">
            <div className="flex flex-col gap-0.5 p-2">
              {incoming.length === 0 && outgoing.length === 0 && (
                <EmptyHint text="Nothing pending." />
              )}
              {incoming.length > 0 && <SectionLabel text={`Incoming - ${incoming.length}`} />}
              {incoming.map((f) => (
                <FriendRow key={f.id} friendship={f} myId={myId} kind="incoming" />
              ))}
              {outgoing.length > 0 && <SectionLabel text={`Outgoing - ${outgoing.length}`} />}
              {outgoing.map((f) => (
                <FriendRow key={f.id} friendship={f} myId={myId} kind="outgoing" />
              ))}
            </div>
          </ScrollArea>
        </TabsContent>
      </Tabs>
    </div>
  );
}

function SectionLabel({ text }: { text: string }) {
  return (
    <p className="px-2 pt-3 pb-1 text-[11px] font-medium tracking-wide text-muted-foreground uppercase">
      {text}
    </p>
  );
}

function EmptyHint({ text }: { text: string }) {
  return <div className="mx-2 my-4 rounded-lg border border-dashed border-white/[0.13] bg-muted/15 px-4 py-8 text-center text-xs leading-relaxed text-muted-foreground/75">{text}</div>;
}

function FriendRow({
  friendship,
  myId,
  kind,
}: {
  friendship: Friendship;
  myId: string;
  kind: "accepted" | "incoming" | "outgoing";
}) {
  const otherId =
    friendship.requester_id === myId ? friendship.addressee_id : friendship.requester_id;
  const profile = useProfiles((s) => s.byId[otherId]);
  const online = useIsOnline(otherId);

  return (
    <div className="flex items-center gap-2.5 rounded-lg border border-transparent px-2.5 py-2.5 transition-colors hover:border-white/[0.09] hover:bg-muted/45">
      <UserAvatar profile={profile} online={kind === "accepted" ? online : undefined} />
      <div className="min-w-0 flex-1 leading-tight">
        <p className="truncate text-sm font-medium">{profile?.display_name ?? "..."}</p>
        <p className="truncate text-xs text-muted-foreground">@{profile?.username ?? ""}</p>
      </div>
      <div className="flex gap-1">
        {kind === "accepted" && (
          <>
            <IconAction
              label="Message"
              onClick={() => void openConversationWith(otherId, myId)}
            >
              <MessageSquareIcon />
            </IconAction>
            <IconAction
              label="Remove friend"
              destructive
              onClick={() => void useFriends.getState().removeFriendship(friendship.id)}
            >
              <UserMinusIcon />
            </IconAction>
          </>
        )}
        {kind === "incoming" && (
          <>
            <IconAction
              label="Accept"
              onClick={() => void useFriends.getState().accept(friendship.id)}
            >
              <CheckIcon />
            </IconAction>
            <IconAction
              label="Decline"
              destructive
              onClick={() => void useFriends.getState().removeFriendship(friendship.id)}
            >
              <XIcon />
            </IconAction>
          </>
        )}
        {kind === "outgoing" && (
          <IconAction
            label="Cancel request"
            destructive
            onClick={() => void useFriends.getState().removeFriendship(friendship.id)}
          >
            <XIcon />
          </IconAction>
        )}
      </div>
    </div>
  );
}

function IconAction({
  label,
  destructive,
  onClick,
  children,
}: {
  label: string;
  destructive?: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label={label}
          className={destructive ? "text-muted-foreground hover:text-destructive" : "text-muted-foreground"}
          onClick={onClick}
        >
          {children}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}

async function openConversationWith(friendId: string, myId: string) {
  const find = () =>
    useChat
      .getState()
      .conversations.find(
        (c) =>
          (c.user1_id === friendId && c.user2_id === myId) ||
          (c.user1_id === myId && c.user2_id === friendId)
      );

  let conv = find();
  if (!conv) {
    await useChat.getState().loadConversations();
    conv = find();
  }
  if (conv) {
    useChat.getState().openConversation(conv.id);
  } else {
    toast.error("Couldn't find that conversation yet. Try again in a moment.");
  }
}

