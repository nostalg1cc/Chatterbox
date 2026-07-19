import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

import { Titlebar } from "@/components/titlebar";
import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { AuthScreen } from "@/features/auth/auth-screen";
import { BootScreen } from "@/features/auth/boot-screen";
import { MainApp } from "@/features/main/main-app";
import { applyWindowMaterial, isTauri } from "@/lib/tauri";
import { useAuth } from "@/stores/auth";
import { usePreferences } from "@/stores/preferences";

export default function App() {
  const status = useAuth((s) => s.status);
  const windowMaterial = usePreferences((s) => s.windowMaterial);
  const acrylicDim = usePreferences((s) => s.acrylicDim);
  const backdropDim = windowMaterial === "acrylic" ? acrylicDim : 12;

  useEffect(() => {
    // One-time preference migration: Mica is the default material going forward.
    if (window.localStorage.getItem("dislight-mica-default-v1") !== "1") {
      usePreferences.getState().setPreference("windowMaterial", "mica");
      window.localStorage.setItem("dislight-mica-default-v1", "1");
    }
    useAuth.getState().init();
    void applyWindowMaterial(
      usePreferences.getState().windowMaterial,
      usePreferences.getState().acrylicDim
    );
  }, []);

  useEffect(() => {
    void applyWindowMaterial(windowMaterial, acrylicDim);
  }, [windowMaterial, acrylicDim]);
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
        // Updates are intentionally quiet at startup. The About page remains
        // available for a manual retry if a network or release check fails.
        console.warn("Automatic update failed", error);
      }
    })();

    return () => {
      disposed = true;
    };
  }, []);

  return (
    <TooltipProvider delayDuration={300}>
      <div
        className={isTauri ? "app-frame relative h-full" : "app-frame relative h-full bg-black"}
        style={isTauri ? { backgroundColor: "rgb(0 0 0 / " + backdropDim + "%)" } : undefined}
      >
        <Titlebar />
        <div className="h-full min-h-0">
          {status === "booting" ? (
            <BootScreen />
          ) : status === "signedOut" ? (
            <AuthScreen />
          ) : (
            <MainApp />
          )}
        </div>
      </div>
      <Toaster position="bottom-right" />
    </TooltipProvider>
  );
}

