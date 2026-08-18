import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { isTauri } from "@/lib/tauri";

export type UpdateStatus = "idle" | "checking" | "current" | "available" | "downloading" | "error";

// Separate from src/lib/updater.ts's startup/manual banner flow - this is
// the About tab's own inline state machine (progress bar drawn in the tab
// itself rather than a top banner), used only while Settings is open.
export function useUpdateChecker() {
  const [status, setStatus] = useState<UpdateStatus>("idle");
  const [availableVersion, setAvailableVersion] = useState<string | null>(null);
  const [pendingUpdate, setPendingUpdate] = useState<Update | null>(null);
  const [progress, setProgress] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  const checkForUpdates = async () => {
    if (!isTauri) { setStatus("current"); return; }
    setStatus("checking");
    setProgress(null);
    setError(null);
    try {
      const update = await check();
      setPendingUpdate(update);
      setAvailableVersion(update?.version ?? null);
      setStatus(update ? "available" : "current");
    } catch (err) {
      setPendingUpdate(null);
      setError(err instanceof Error ? err.message : "Unknown updater error.");
      setStatus("error");
    }
  };

  const installUpdate = async () => {
    if (!isTauri) return;
    setStatus("downloading");
    setProgress(0);
    setError(null);
    let receivedBytes = 0;
    let totalBytes = 0;
    try {
      const update = pendingUpdate ?? await check();
      if (!update) {
        setStatus("current");
        setProgress(null);
        return;
      }
      setPendingUpdate(update);
      setAvailableVersion(update.version);
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          totalBytes = event.data.contentLength ?? 0;
        } else if (event.event === "Progress") {
          receivedBytes += event.data.chunkLength;
          if (totalBytes > 0) setProgress(Math.min(100, Math.round((receivedBytes / totalBytes) * 100)));
        } else if (event.event === "Finished") {
          setProgress(100);
        }
      });
      // Give the filesystem/AV a moment to release the just-replaced exe
      // before relaunching - see the matching comment in App.tsx.
      await new Promise((resolve) => window.setTimeout(resolve, 1500));
      await invoke("restart_app");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown updater error.");
      setStatus("error");
      setProgress(null);
    }
  };

  return { status, availableVersion, progress, error, checkForUpdates, installUpdate };
}
