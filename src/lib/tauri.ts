import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { WindowMaterial } from "@/stores/preferences";

export const isTauri = "__TAURI_INTERNALS__" in window;

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