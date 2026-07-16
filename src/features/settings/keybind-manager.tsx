import { useEffect } from "react";
import { eventKeybind, globalKeybind } from "@/lib/keybinds";
import { isTauri } from "@/lib/tauri";
import { usePreferences } from "@/stores/preferences";
import { useVoice } from "@/stores/voice";

let globalShortcutGeneration = 0;

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

      if (binding === preferences.keybinds.toggleMute) {
        event.preventDefault();
        voice.toggleMute();
      } else if (binding === preferences.keybinds.toggleDeafen) {
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
    if (!isTauri) return;

    const generation = ++globalShortcutGeneration;
    const bindings = globalVoiceShortcuts
      ? [
          { binding: globalKeybind(keybinds.toggleMute), action: "mute" as const },
          { binding: globalKeybind(keybinds.toggleDeafen), action: "deafen" as const },
        ].filter((entry): entry is { binding: string; action: "mute" | "deafen" } => Boolean(entry.binding))
      : [];

    void import("@tauri-apps/plugin-global-shortcut")
      .then(async ({ register, unregisterAll }) => {
        // Reconcile from one current generation. Do not unregister in effect cleanup:
        // React Strict Mode can otherwise remove a freshly registered native shortcut.
        await unregisterAll();
        if (generation !== globalShortcutGeneration) return;

        for (const entry of bindings) {
          await register(entry.binding, (event) => {
            if (event.state !== "Pressed") return;
            if (entry.action === "mute") useVoice.getState().toggleMute();
            if (entry.action === "deafen") useVoice.getState().toggleDeafen();
          });
        }
      })
      .catch((error) => console.warn("Global shortcuts unavailable", error));

    return () => {
      // The following effect invocation performs the reconciliation. Keeping this
      // cleanup inert prevents an asynchronous stale unregister from winning.
    };
  }, [globalVoiceShortcuts, keybinds]);

  return null;
}