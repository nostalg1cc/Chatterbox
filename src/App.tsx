import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { V3Shell } from "@/features/v3-shell/v3-shell";
import { isTauri } from "@/lib/tauri";
import { useAuth } from "@/stores/auth";
import { usePresence } from "@/stores/presence";
import { useVoice } from "@/stores/voice";

export default function App() {
  const userId = useAuth((state) => state.userId);

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
        if (!disposed) await invoke("restart_app");
      } catch (error) {
        console.warn("Automatic update failed", error);
      }
    })();
    return () => { disposed = true; };
  }, []);

  return <TooltipProvider delayDuration={300}><V3Shell /><Toaster position="bottom-right" /></TooltipProvider>;
}
