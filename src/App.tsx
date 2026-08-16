import { useEffect } from "react";
import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { GlobalTitlebar } from "@/components/titlebar";
import { V3Shell } from "@/features/v3-shell/v3-shell";
import { AuthScreen } from "@/features/auth/auth-screen";
import { KeybindManager } from "@/features/settings/keybind-manager";
import { TopAlert } from "@/features/v3-shell/components/TopAlert";
import { checkForUpdateAndNotify, UPDATED_VERSION_KEY } from "@/lib/updater";
import { useAlerts } from "@/stores/alerts";
import { useAuth } from "@/stores/auth";
import { useFriends } from "@/stores/friends";
import { usePresence } from "@/stores/presence";
import { useVoice } from "@/stores/voice";

export default function App() {
  const userId = useAuth((state) => state.userId);
  const status = useAuth((state) => state.status);
  const activeAlert = useAlerts((state) => state.active);
  const dismissAlert = useAlerts((state) => state.dismiss);

  useEffect(() => { useAuth.getState().init(); }, []);
  useEffect(() => { if (!userId) return; return useVoice.getState().init(userId); }, [userId]);
  useEffect(() => { if (!userId) return; return usePresence.getState().join(userId); }, [userId]);
  useEffect(() => {
    if (!userId) return;
    void useFriends.getState().load();
    return useFriends.getState().subscribe(userId);
  }, [userId]);
  useEffect(() => {
    // The app process fully restarts after installing an update, so there's
    // no in-memory way to know "we just updated" on the next launch - the
    // version gets stashed in localStorage right before the restart below,
    // and this picks it up the first time the fresh process starts up.
    const updatedVersion = window.localStorage.getItem(UPDATED_VERSION_KEY);
    if (updatedVersion) {
      window.localStorage.removeItem(UPDATED_VERSION_KEY);
      useAlerts.getState().show({ severity: "neutral", message: `App got updated to version ${updatedVersion}.` });
    }
  }, []);
  // Checks on startup, but only ever notifies - nothing installs without
  // an explicit "Update now" click. The About page and /update remain
  // available any time for an on-demand check too.
  useEffect(() => { void checkForUpdateAndNotify(); }, []);

  return (
    <TooltipProvider delayDuration={300}>
      <GlobalTitlebar />
      <KeybindManager />
      {/* V3Shell renders its own top-alert-region, inside .stage's isolated
          stacking context, so the banner correctly sits behind the header
          pills there (z-index 3 vs 5) instead of covering them. Outside
          V3Shell (auth, initial load) there's no header to layer under, so
          this simpler top-level instance covers that case instead. */}
      {status !== "signedIn" && activeAlert && (
        <div className="top-alert-region" aria-live="polite" aria-atomic="true">
          <TopAlert
            id={activeAlert.id}
            message={activeAlert.message}
            severity={activeAlert.severity}
            icon={activeAlert.icon}
            actions={activeAlert.actions}
            onDismiss={dismissAlert}
          />
        </div>
      )}
      {status === "signedIn" ? (
        <V3Shell />
      ) : status === "signedOut" ? (
        <AuthScreen />
      ) : (
        <div className="flex h-screen items-center justify-center bg-background" />
      )}
      <Toaster position="bottom-right" />
    </TooltipProvider>
  );
}
