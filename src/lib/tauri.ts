import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

/** True when running inside the Tauri shell (false in a plain browser tab). */
export const isTauri = "__TAURI_INTERNALS__" in window;

export function appWindow() {
  return getCurrentWindow();
}

/**
 * Applies the .no-mica class when the native Mica material is unavailable
 * (browser, or Windows 10 and earlier) so the app gets a solid background.
 */
export async function applyMicaClass(): Promise<void> {
  let mica = false;
  if (isTauri) {
    try {
      mica = await invoke<boolean>("is_mica_supported");
    } catch {
      mica = false;
    }
  }
  document.documentElement.classList.toggle("no-mica", !mica);
}
