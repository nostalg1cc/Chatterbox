import { useEffect } from "react";
import { Titlebar } from "@/components/titlebar";
import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { AuthScreen } from "@/features/auth/auth-screen";
import { BootScreen } from "@/features/auth/boot-screen";
import { MainApp } from "@/features/main/main-app";
import { applyMicaClass } from "@/lib/tauri";
import { useAuth } from "@/stores/auth";

export default function App() {
  const status = useAuth((s) => s.status);

  useEffect(() => {
    useAuth.getState().init();
    void applyMicaClass();
  }, []);

  return (
    <TooltipProvider delayDuration={300}>
      <div className="flex h-full flex-col">
        <Titlebar />
        <div className="min-h-0 flex-1">
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
