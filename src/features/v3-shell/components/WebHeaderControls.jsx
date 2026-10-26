import { PanelRight, Settings } from "lucide-react";
import { isTauri } from "@/lib/tauri";
import { useAuth } from "@/stores/auth";
import { useChat } from "@/stores/chat";
import { ActionButton } from "./ActionButton";

/** App actions normally supplied by the native Windows caption buttons. */
export function WebHeaderControls() {
  const signedIn = useAuth((state) => state.status === "signedIn");
  const onboarding = useAuth((state) => Boolean(state.profile && !state.profile.onboarding_completed_at));
  const view = useChat((state) => state.view);
  const mediaOpen = useChat((state) => state.mediaSidebarOpen);
  if (isTauri || !signedIn || onboarding) return null;
  return <>
    {view === "chat" && <ActionButton icon={PanelRight} label={mediaOpen ? "Hide media sidebar" : "Show media sidebar"} className="web-header-control" aria-pressed={mediaOpen} onClick={() => useChat.getState().toggleMediaSidebar()} />}
    <ActionButton icon={Settings} label={view === "settings" ? "Close settings" : "Settings"} className="web-header-control" aria-pressed={view === "settings"} onClick={() => view === "settings" ? useChat.getState().closeSettings() : useChat.getState().openSettings()} />
  </>;
}
