import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { CopyRegular, DismissRegular, PanelRightRegular, SettingsRegular, SquareRegular, SubtractRegular } from "@fluentui/react-icons";
import { Button } from "@/components/ui/button";
import { appWindow, isTauri } from "@/lib/tauri";
import { useAuth } from "@/stores/auth";
import { useChat } from "@/stores/chat";

export function WindowControls() {
  const [maximized, setMaximized] = useState(false);
  const signedIn = useAuth((state) => state.status === "signedIn");
  const onboarding = useAuth((state) => Boolean(state.profile && !state.profile.onboarding_completed_at));
  const chatView = useChat((state) => state.view);
  const mediaSidebarOpen = useChat((state) => state.mediaSidebarOpen);
  const toggleMediaSidebar = useChat((state) => state.toggleMediaSidebar);
  const openSettings = useChat((state) => state.openSettings);
  const closeSettings = useChat((state) => state.closeSettings);

  useEffect(() => {
    if (!isTauri) return;
    const win = appWindow();
    let unlisten: (() => void) | undefined;
    let disposed = false;
    const update = () => void win.isMaximized().then((value) => { if (!disposed) setMaximized(value); });
    update();
    void win.onResized(update).then((fn) => { if (disposed) fn(); else unlisten = fn; });
    return () => { disposed = true; unlisten?.(); };
  }, []);

  if (!isTauri) return null;

  return (
    <div className="window-controls flex h-full items-center gap-0" aria-label="Window controls">
      {signedIn && !onboarding && chatView === "chat" && (
        <Button
          variant="ghost"
          aria-label={mediaSidebarOpen ? "Hide media sidebar" : "Show media sidebar"}
          className={"window-control" + (mediaSidebarOpen ? " window-control-active" : "")}
          onClick={toggleMediaSidebar}
        >
          <PanelRightRegular />
        </Button>
      )}
      {signedIn && !onboarding && (
        <Button
          variant="ghost"
          aria-label={chatView === "settings" ? "Close settings" : "Settings"}
          className={"window-control" + (chatView === "settings" ? " window-control-active" : "")}
          onClick={() => (chatView === "settings" ? closeSettings() : openSettings())}
        >
          <SettingsRegular />
        </Button>
      )}
      <Button variant="ghost" aria-label="Minimize" className="window-control" onClick={() => void appWindow().minimize()}>
        <SubtractRegular />
      </Button>
      <Button variant="ghost" aria-label={maximized ? "Restore" : "Maximize"} className="window-control" onClick={() => void appWindow().toggleMaximize()}>
        {maximized ? <CopyRegular /> : <SquareRegular />}
      </Button>
      <Button variant="ghost" aria-label="Close" className="window-control window-control-close" onClick={() => void appWindow().close()}>
        <DismissRegular />
      </Button>
    </div>
  );
}

// Portaled straight to <body> so the window controls stay clickable above
// every full-screen view/overlay (Settings, Dashboard, the media lightbox,
// etc) - those live inside .stage, which uses isolation:isolate, so no
// z-index set inside that context can out-rank something outside it. This is
// just the small top-right button cluster, not the drag strip: a full-width
// drag region up here would sit on top of in-app UI (the call button, nav
// controls, ...) that normally lives in that same top band, making it
// unclickable. Settings/Dashboard stay draggable by never covering the drag
// strip in the first place (see SettingsView/V3Dashboard); V3Lightbox is the
// one overlay that does need to visually cover it, so it has its own scoped
// drag strip instead.
export function GlobalTitlebar() {
  if (!isTauri || typeof document === "undefined") return null;
  return createPortal(<div className="global-window-controls"><WindowControls /></div>, document.body);
}
