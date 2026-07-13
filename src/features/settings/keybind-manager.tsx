import { useEffect } from "react";
import { eventKeybind, globalKeybind } from "@/lib/keybinds";
import { isTauri } from "@/lib/tauri";
import { usePreferences } from "@/stores/preferences";
import { useVoice } from "@/stores/voice";

export function KeybindManager() {
  const globalVoiceShortcuts = usePreferences((state) => state.globalVoiceShortcuts);
  const keybinds = usePreferences((state) => state.keybinds);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.repeat || event.defaultPrevented) return;
      const binding = eventKeybind(event);
      if (!binding) return;
      const preferences = usePreferences.getState();
      const voice = useVoice.getState();
      const nativeGlobal = isTauri && preferences.globalVoiceShortcuts;

      if (binding === preferences.keybinds.toggleMute) {
        if (nativeGlobal) return;
        event.preventDefault();
        voice.toggleMute();
      } else if (binding === preferences.keybinds.toggleDeafen) {
        if (nativeGlobal) return;
        event.preventDefault();
        voice.toggleDeafen();
      } else if (binding === preferences.keybinds.leaveVoice && voice.activeConversationId) {
        event.preventDefault();
        void voice.leave();
      } else if (binding === preferences.keybinds.toggleScreenShare && voice.activeConversationId) {
        event.preventDefault();
        void (voice.sharingScreen ? voice.stopScreenShare() : voice.startScreenShare());
      } else if (binding === preferences.keybinds.openSoundboard && voice.activeConversationId) {
        event.preventDefault();
        window.dispatchEvent(new CustomEvent("dislight:open-soundboard"));
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  useEffect(() => {
    if (!isTauri || !globalVoiceShortcuts) return;
    let disposed = false;
    const bindings = [
      { binding: globalKeybind(keybinds.toggleMute), action: "mute" },
      { binding: globalKeybind(keybinds.toggleDeafen), action: "deafen" },
    ].filter((entry): entry is { binding: string; action: "mute" | "deafen" } => Boolean(entry.binding));

    void import("@tauri-apps/plugin-global-shortcut").then(async ({ register, unregisterAll }) => {
      await unregisterAll();
      if (disposed || bindings.length === 0) return;
      await register(bindings.map((entry) => entry.binding), (event) => {
        if (event.state !== "Pressed") return;
        const action = bindings.find((entry) => entry.binding.toLowerCase() === event.shortcut.toLowerCase())?.action;
        if (action === "mute") useVoice.getState().toggleMute();
        if (action === "deafen") useVoice.getState().toggleDeafen();
      });
    }).catch((error) => console.warn("Global shortcuts unavailable", error));

    return () => {
      disposed = true;
      void import("@tauri-apps/plugin-global-shortcut")
        .then(({ unregisterAll }) => unregisterAll())
        .catch(() => undefined);
    };
  }, [globalVoiceShortcuts, keybinds]);

  return null;
}