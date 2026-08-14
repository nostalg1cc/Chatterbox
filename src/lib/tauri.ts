import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { WindowMaterial } from "@/stores/preferences";

export const isTauri = "__TAURI_INTERNALS__" in window;

// Flags the PWA/web build at the root as early as possible (this module is
// one of the first things imported) so CSS can size things up there
// specifically - the desktop app's compact controls read as too small once
// you're on a browser/touch-scale display instead of sitting close to the
// screen.
if (typeof document !== "undefined") {
  document.documentElement.classList.toggle("is-web", !isTauri);
}

export function appWindow() {
  return getCurrentWindow();
}

export async function applyWindowMaterial(
  material: WindowMaterial,
  acrylicDim: number
): Promise<void> {
  let applied = false;
  if (isTauri) {
    try {
      applied = await invoke<boolean>("set_window_material", { material, acrylicDim });
    } catch {
      applied = false;
    }
  }
  document.documentElement.classList.toggle("no-mica", !applied);
  document.documentElement.classList.toggle("acrylic", applied && material === "acrylic");
}