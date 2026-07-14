import { useEffect } from "react";
import { MessageSquareIcon, UsersIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { WindowControls } from "@/components/titlebar";
import { ChatView } from "@/features/chat/chat-view";
import { ChatSwitcher } from "@/features/chat/chat-switcher";
import { AddFriendDialog } from "@/features/friends/add-friend-dialog";
import { FriendsView } from "@/features/friends/friends-view";
import { KeybindManager } from "@/features/settings/keybind-manager";
import { preloadAppSounds } from "@/lib/app-sounds";
import { purgeExpiredMediaCache } from "@/lib/media-cache";
import { supabase } from "@/lib/supabase";
import { useAuth } from "@/stores/auth";
import { useChat } from "@/stores/chat";
import { useFriends } from "@/stores/friends";
import { usePresence } from "@/stores/presence";
import { useProfiles } from "@/stores/profiles";
import { appWindow, isTauri } from "@/lib/tauri";
import { useVoice } from "@/stores/voice";

let cleanupRequested = false;

export function MainApp() {
  const userId = useAuth((state) => state.userId);
  const view = useChat((state) => state.view);
  const activeId = useChat((state) => state.activeId);

  useEffect(() => {
    if (!userId) return;

    preloadAppSounds();
    const ownProfile = useAuth.getState().profile;
    if (ownProfile) useProfiles.getState().put([ownProfile]);

    void useFriends.getState().load();
    void useChat.getState().loadConversations();
    void purgeExpiredMediaCache().catch((error) => {
      console.warn("Local media cache cleanup failed", error);
    });

    if (!cleanupRequested) {
      cleanupRequested = true;
      void supabase.functions.invoke("purge-chat-media", {
        body: { mode: "scheduled" },
      });
    }

    const unsubChat = useChat.getState().subscribe();
    const unsubFriends = useFriends.getState().subscribe(userId);
    const unsubProfiles = useProfiles.getState().subscribe((profile) => {
      useAuth.getState().applyProfile(profile);
    });
    const leavePresence = usePresence.getState().join(userId);
    const unsubscribeVoice = useVoice.getState().init(userId);

    return () => {
      unsubChat();
      unsubFriends();
      unsubProfiles();
      leavePresence();
      unsubscribeVoice();
      useChat.getState().reset();
      useProfiles.getState().reset();
    };
  }, [userId]);

  return (
    <div
      className="relative flex h-full min-h-0"
      onMouseDown={(event) => {
        if (!isTauri || event.button !== 0) return;
        const target = event.target as HTMLElement;
        if (target.closest("button, input, textarea, select, a, [role=button]")) return;
        const top = event.currentTarget.getBoundingClientRect().top;
        if (event.clientY - top < 88) void appWindow().startDragging();
      }}
    >
      <KeybindManager />
      <main className="app-shell relative m-0 min-w-0 flex-1 overflow-hidden border-0 bg-transparent">
        {(view === "friends" || !activeId) && (
          <div className="surface-panel floating-surface conversation-dock absolute top-[21px] left-1/2 z-30 flex h-11 w-max max-w-[calc(100%-42px)] -translate-x-1/2 items-center gap-1 p-1 shadow-[0_12px_28px_rgb(0_0_0_/_0.28)]">
            {activeId ? (
              <ChatSwitcher conversationId={activeId} />
            ) : (
              <>
                <Button variant="ghost" size="sm" className="justify-start" onClick={() => useChat.getState().setView("friends")}>
                  <UsersIcon />
                  Friends
                </Button>
                <AddFriendDialog />
                <span className="min-w-0 truncate px-1 text-xs text-muted-foreground">Add a friend to start chatting.</span>
              </>
            )}
          </div>
        )}
        {(view === "friends" || !activeId) && <div className="window-controls-reveal absolute top-[9px] right-[9px] z-40 h-10 w-[108px]"><WindowControls /></div>}
        <div className="h-full min-h-0">
          {view === "friends" ? (
            <FriendsView />
          ) : activeId ? (
            <ChatView key={activeId} conversationId={activeId} />
          ) : (
            <NoConversation />
          )}
        </div>
      </main>
    </div>
  );
}

function NoConversation() {
  return (
    <div className="flex h-full flex-col items-center justify-center px-6 text-center">
      <div className="flex size-12 items-center justify-center rounded-2xl border border-white/[0.12] bg-muted/25 text-muted-foreground">
        <MessageSquareIcon className="size-5" />
      </div>
      <div className="mt-4 max-w-xs">
        <p className="text-sm font-medium text-foreground/85">No conversation selected</p>
        <p className="mt-1.5 text-xs leading-relaxed text-muted-foreground/75">
          Add a friend to start a direct message.
        </p>
      </div>
    </div>
  );
}




