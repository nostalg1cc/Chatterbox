import { useEffect } from "react";
import { MessageSquareIcon } from "lucide-react";
import { WindowControls } from "@/components/titlebar";
import { ChatView } from "@/features/chat/chat-view";
import { Sidebar } from "@/features/chat/sidebar";
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
    <div className="relative flex h-full min-h-0">
      <KeybindManager />
      {(view === "friends" || !activeId) && <div className="absolute top-0 right-0 z-40 h-8"><WindowControls /></div>}
      <Sidebar />
      <main className="mt-[5px] mr-[5px] mb-[5px] min-w-0 flex-1 overflow-hidden rounded-tl-[25px] rounded-tr-[5px] rounded-br-[5px] rounded-bl-[25px] border-[1.25px] border-solid border-white/[0.15] bg-[#0D0D0D]">
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
          Pick a direct message from the sidebar, or add a friend to start a new one.
        </p>
      </div>
    </div>
  );
}




