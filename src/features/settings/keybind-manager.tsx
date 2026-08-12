import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { eventKeybind, globalKeybind } from "@/lib/keybinds";
import { isTauri } from "@/lib/tauri";
import { usePreferences } from "@/stores/preferences";
import { useVoice } from "@/stores/voice";

export function KeybindManager() {
  const globalVoiceShortcuts = usePreferences((state) => state.globalVoiceShortcuts);
  const keybinds = usePreferences((state) => state.keybinds);
  // Actions ("mute"/"deafen") whose global registration failed last time
  // (e.g. claimed by another app) - the in-window shortcut stays live for
  // those instead of silently doing nothing.
  const failedGlobalActionsRef = useRef<Set<string>>(new Set());

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.repeat || event.defaultPrevented) return;
      const binding = eventKeybind(event);
      if (!binding) return;
      const preferences = usePreferences.getState();
      const voice = useVoice.getState();

      if (binding === preferences.keybinds.toggleMute) {
        if (isTauri && preferences.globalVoiceShortcuts && !failedGlobalActionsRef.current.has("mute")) return;
        event.preventDefault();
        voice.toggleMute();
      } else if (binding === preferences.keybinds.toggleDeafen) {
        if (isTauri && preferences.globalVoiceShortcuts && !failedGlobalActionsRef.current.has("deafen")) return;
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
    let unlisten: UnlistenFn | undefined;
    let disposed = false;

    void listen<string>("dislight:global-voice-shortcut", (event) => {
      const voice = useVoice.getState();
      if (!voice.activeConversationId) return;
      if (event.payload === "mute") voice.toggleMute();
      if (event.payload === "deafen") voice.toggleDeafen();
    }).then((stop) => {
      if (disposed) void stop();
      else unlisten = stop;
    }).catch((error) => console.warn("Global shortcut event listener unavailable", error));

    return () => {
      disposed = true;
      if (unlisten) void unlisten();
    };
  }, []);

  useEffect(() => {
    if (!isTauri) return;
    const muteShortcut = globalVoiceShortcuts ? globalKeybind(keybinds.toggleMute) : null;
    const deafenShortcut = globalVoiceShortcuts ? globalKeybind(keybinds.toggleDeafen) : null;
    let disposed = false;

    void invoke<string[]>("configure_global_voice_shortcuts", {
      enabled: globalVoiceShortcuts,
      muteShortcut,
      deafenShortcut,
    }).then((failed) => {
      if (disposed) return;
      failedGlobalActionsRef.current = new Set(failed);
      if (failed.length > 0) {
        toast.warning(
          `Could not register the global ${failed.join("/")} shortcut - it's already used by another app. It'll still work while Nitro is focused.`
        );
      }
    }).catch((error) => {
      if (!disposed) {
        // The whole call failing (not just one shortcut) - fall back to
        // local handling for both rather than leaving neither working.
        failedGlobalActionsRef.current = new Set(["mute", "deafen"]);
        console.warn("Global shortcuts unavailable", error);
        toast.error(typeof error === "string" ? error : "Could not register global voice shortcuts.");
      }
    });

    return () => { disposed = true; };
  }, [globalVoiceShortcuts, keybinds]);

  return null;
}