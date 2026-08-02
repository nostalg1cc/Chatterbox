import { Volume2, VolumeX } from "lucide-react";
import { usePreferences } from "@/stores/preferences";
import { ActionButton } from "./ActionButton";

export function SoundToggleButton() {
  const soundsEnabled = usePreferences((state) => state.interfaceSounds);
  const setPreference = usePreferences((state) => state.setPreference);
  const SoundIcon = soundsEnabled ? Volume2 : VolumeX;

  return (
    <ActionButton
      icon={SoundIcon}
      label={soundsEnabled ? "Mute interface sounds" : "Enable interface sounds"}
      aria-pressed={soundsEnabled}
      className={soundsEnabled ? "sound-toggle-button" : "sound-toggle-button is-muted"}
      onClick={() => setPreference("interfaceSounds", !soundsEnabled)}
    />
  );
}
