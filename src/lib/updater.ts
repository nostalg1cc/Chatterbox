import { invoke } from "@tauri-apps/api/core";
import type { Update } from "@tauri-apps/plugin-updater";
import { isTauri } from "@/lib/tauri";
import { useAlerts } from "@/stores/alerts";

// The app process fully restarts after installing an update, so there's no
// in-memory way to know "we just updated" on the next launch - the version
// gets stashed here right before the restart, and App.tsx picks it up the
// first time the fresh process starts back up.
export const UPDATED_VERSION_KEY = "dislight-updated-version";

async function installAndRestart(update: Update): Promise<void> {
  await update.downloadAndInstall();
  window.localStorage.setItem(UPDATED_VERSION_KEY, update.version);
  // The NSIS installer just replaced the exe on disk - give the
  // filesystem/AV a moment to release it before relaunching, or the new
  // process can read a still-locked/partially-flushed binary and crash
  // immediately (observed as a 0xc0000409 fail-fast on launch).
  await new Promise((resolve) => window.setTimeout(resolve, 1500));
  await invoke("restart_app");
}

/** Startup check - surfaces an "Update available" banner with its own
 * Dismiss/Update now actions instead of installing anything silently. Errors
 * are swallowed (unlike checkForUpdateManually): a background check failing
 * shouldn't interrupt anyone, and About/`/update` are still there to retry. */
export async function checkForUpdateAndNotify(): Promise<void> {
  if (!isTauri) return;
  try {
    const { check } = await import("@tauri-apps/plugin-updater");
    const update = await check();
    if (!update) return;
    useAlerts.getState().show({
      severity: "neutral",
      message: `Update ${update.version} is available.`,
      actions: [
        { label: "Dismiss" },
        { label: "Update now", confirm: true, onClick: () => void installAndRestart(update) },
      ],
    });
  } catch (error) {
    console.warn("Update check failed", error);
  }
}

/** User-triggered check (e.g. the /update chat command) - always reports back via the alert banner. */
export async function checkForUpdateManually(): Promise<void> {
  if (!isTauri) {
    useAlerts.getState().show({ severity: "neutral", message: "Updates aren't available in the web build." });
    return;
  }
  useAlerts.getState().show({ severity: "neutral", message: "Checking for updates…" });
  try {
    const { check } = await import("@tauri-apps/plugin-updater");
    const update = await check();
    if (!update) {
      useAlerts.getState().show({ severity: "neutral", message: "You're already on the latest version." });
      return;
    }
    useAlerts.getState().show({
      severity: "neutral",
      message: `Update ${update.version} is available.`,
      actions: [
        { label: "Later" },
        { label: "Install", confirm: true, onClick: () => void installAndRestart(update) },
      ],
    });
  } catch (error) {
    useAlerts.getState().show({ severity: "danger", message: error instanceof Error ? error.message : "Could not check for updates." });
  }
}
