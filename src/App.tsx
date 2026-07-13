import { useEffect } from "react";
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
    useAuth.getState().init();
    void applyWindowMaterial(
      usePreferences.getState().windowMaterial,
      usePreferences.getState().acrylicDim
    );
  }, []);

  useEffect(() => {
    void applyWindowMaterial(windowMaterial, acrylicDim);
  }, [windowMaterial, acrylicDim]);

  return (
    <TooltipProvider delayDuration={300}>
      <div
        className={isTauri ? "relative h-full" : "relative h-full bg-black"}
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

