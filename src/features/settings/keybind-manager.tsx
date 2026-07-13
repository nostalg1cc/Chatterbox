import { useEffect } from "react";
import { eventKeybind } from "@/lib/keybinds";
import { usePreferences } from "@/stores/preferences";
import { useVoice } from "@/stores/voice";

export function KeybindManager() {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.repeat || event.defaultPrevented) return;
      const binding = eventKeybind(event);
      if (!binding) return;
      const { keybinds } = usePreferences.getState();
      const voice = useVoice.getState();

      if (binding === keybinds.toggleMute) {
        event.preventDefault();
        voice.toggleMute();
      } else if (binding === keybinds.toggleDeafen) {
        event.preventDefault();
        voice.toggleDeafen();
      } else if (binding === keybinds.leaveVoice && voice.activeConversationId) {
        event.preventDefault();
        void voice.leave();
      } else if (binding === keybinds.toggleScreenShare && voice.activeConversationId) {
        event.preventDefault();
        void (voice.sharingScreen ? voice.stopScreenShare() : voice.startScreenShare());
      } else if (binding === keybinds.openSoundboard && voice.activeConversationId) {
        event.preventDefault();
        window.dispatchEvent(new CustomEvent("dislight:open-soundboard"));
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  return null;
}