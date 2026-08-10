import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { V3Shell } from "@/features/v3-shell/v3-shell";
import { AuthScreen } from "@/features/auth/auth-screen";
import { KeybindManager } from "@/features/settings/keybind-manager";
import { isTauri } from "@/lib/tauri";
import { useAuth } from "@/stores/auth";
import { usePresence } from "@/stores/presence";
import { useVoice } from "@/stores/voice";

export default function App() {
  const userId = useAuth((state) => state.userId);
  const status = useAuth((state) => state.status);

  useEffect(() => { useAuth.getState().init(); }, []);
  useEffect(() => { if (!userId) return; return useVoice.getState().init(userId); }, [userId]);
  useEffect(() => { if (!userId) return; return usePresence.getState().join(userId); }, [userId]);
  useEffect(() => {
    if (!isTauri) return;
    let disposed = false;
    void (async () => {
      try {
        const { check } = await import("@tauri-apps/plugin-updater");
        const update = await check();
        if (!update || disposed) return;
        await update.downloadAndInstall();
        if (disposed) return;
        // The NSIS installer just replaced the exe on disk - give the
        // filesystem/AV a moment to release it before relaunching, or the
        // new process can read a still-locked/partially-flushed binary and
        // crash immediately (observed as a 0xc0000409 fail-fast on launch).
        await new Promise((resolve) => window.setTimeout(resolve, 1500));
        if (!disposed) await invoke("restart_app");
      } catch (error) {
        console.warn("Automatic update failed", error);
      }
    })();
    return () => { disposed = true; };
  }, []);

  return (
    <TooltipProvider delayDuration={300}>
      <KeybindManager />
      {status === "signedIn" ? (
        <V3Shell />
      ) : status === "signedOut" ? (
        <AuthScreen />
      ) : (
        <div className="flex h-screen items-center justify-center bg-background" />
      )}
      <Toaster position="top-center" />
    </TooltipProvider>
  );
}
