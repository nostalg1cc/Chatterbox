import { useEffect, useState } from "react";
import { eventKeybind, keybindLabel } from "@/lib/keybinds";
import type { KeybindPreferences } from "@/stores/preferences";
import { usePreferences } from "@/stores/preferences";
import { useAlerts } from "@/stores/alerts";

const KEYBIND_LABELS: Record<keyof KeybindPreferences, string> = {
  toggleMute: "Toggle mute",
  toggleDeafen: "Toggle deafen",
  leaveVoice: "Leave voice",
  toggleScreenShare: "Toggle screen share",
  openSoundboard: "Open soundboard",
};

export function KeybindRow({
  action,
  value,
  onChange,
}: {
  action: keyof KeybindPreferences;
  value: string;
  onChange: (value: string) => void;
}) {
  const [recording, setRecording] = useState(false);
  const globalVoiceShortcuts = usePreferences((state) => state.globalVoiceShortcuts);
  const isGlobalCapable = action === "toggleMute" || action === "toggleDeafen";

  useEffect(() => {
    if (!recording) return;
    const capture = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopImmediatePropagation();
      if (event.key === "Escape") {
        setRecording(false);
        return;
      }
      if (event.key === "Backspace" || event.key === "Delete") {
        onChange("");
        setRecording(false);
        return;
      }
      const binding = eventKeybind(event);
      if (!binding) return;
      // Bare keys (no modifier) can't be registered as an OS-wide global
      // shortcut - globalKeybind() already skips sending those to Rust, so
      // this only works while Nitro's window is focused. That's fine for
      // app-only actions, but worth a heads-up for mute/deafen when global
      // shortcuts are turned on, since the expectation there is "works
      // anywhere".
      if (isGlobalCapable && globalVoiceShortcuts && !binding.includes("+")) {
        useAlerts.getState().show({
          severity: "warning",
          message: "That shortcut will only work while Nitro is focused - bare keys can't be registered as a system-wide global shortcut.",
        });
      }
      onChange(binding);
      setRecording(false);
    };
    window.addEventListener("keydown", capture, true);
    return () => window.removeEventListener("keydown", capture, true);
  }, [onChange, recording, isGlobalCapable, globalVoiceShortcuts]);

  return (
    <div className="v3-settings__row">
      <div className="v3-settings__row-copy">
        <p className="v3-settings__row-title">{KEYBIND_LABELS[action]}</p>
        <p className="v3-settings__row-desc">
          {recording
            ? "Waiting for a key combination…"
            : action === "openSoundboard"
              ? "Available while connected to voice."
              : "Press Escape to cancel or Backspace to clear."}
        </p>
      </div>
      <button
        type="button"
        className={"v3-settings__ghost-button v3-settings__keybind-button" + (recording ? " is-recording" : "")}
        onClick={() => setRecording(true)}
      >
        {recording ? "Press a combination…" : value ? keybindLabel(value) : "Not set"}
      </button>
    </div>
  );
}
