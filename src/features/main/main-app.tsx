import { useEffect } from "react";
import { MessageSquareIcon } from "lucide-react";
import { ChatView } from "@/features/chat/chat-view";
import { Sidebar } from "@/features/chat/sidebar";
import { FriendsView } from "@/features/friends/friends-view";
import { useAuth } from "@/stores/auth";
import { useChat } from "@/stores/chat";
import { useFriends } from "@/stores/friends";
import { usePresence } from "@/stores/presence";

export function MainApp() {
  const userId = useAuth((s) => s.userId);
  const view = useChat((s) => s.view);
  const activeId = useChat((s) => s.activeId);

  useEffect(() => {
    if (!userId) return;
    void useFriends.getState().load();
    void useChat.getState().loadConversations();
    const unsubChat = useChat.getState().subscribe();
    const unsubFriends = useFriends.getState().subscribe(userId);
    const leavePresence = usePresence.getState().join(userId);
    return () => {
      unsubChat();
      unsubFriends();
      leavePresence();
      useChat.getState().reset();
    };
  }, [userId]);

  return (
    <div className="flex h-full min-h-0">
      <Sidebar />
      {/* The chat pane is the solid surface; the sidebar stays transparent for Mica. */}
      <main className="min-w-0 flex-1 border-l bg-background">
        {view === "friends" ? (
          <FriendsView />
        ) : activeId ? (
          <ChatView key={activeId} conversationId={activeId} />
        ) : (
          <NoConversation />
        )}
      </main>
    </div>
  );
}

function NoConversation() {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
      <MessageSquareIcon className="size-8 text-muted-foreground/40" />
      <div>
        <p className="text-sm font-medium text-muted-foreground">No conversation selected</p>
        <p className="mt-1 text-xs text-muted-foreground/70">
          Pick one from the sidebar, or add a friend to start talking.
        </p>
      </div>
    </div>
  );
}
