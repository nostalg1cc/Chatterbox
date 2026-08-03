import { ChevronDown, Mic, MicOff } from "lucide-react";
import { usePreferences } from "@/stores/preferences";
import { AudioDeviceMenu } from "./AudioDeviceMenu";
import { useUiSounds } from "../hooks/useUiSounds";

const MENU_ACTION_WIDTH = 22;

export function MicrophoneToggleDropdown({ isMuted, onToggle, isMenuOpen, onMenuOpenChange }) {
  const sounds = useUiSounds();
  const inputVolume = usePreferences((state) => state.inputVolume);
  const setPreference = usePreferences((state) => state.setPreference);
  const Icon = isMuted ? MicOff : Mic;

  return (
    <div className="audio-control">
      <button
        type="button"
        aria-label={isMuted ? "Enable microphone" : "Mute microphone"}
        aria-pressed={isMuted}
        aria-haspopup="menu"
        aria-expanded={isMenuOpen}
        className={`icon-button microphone-button${isMuted ? " is-muted" : ""}`}
        onClick={(event) => {
          const rect = event.currentTarget.getBoundingClientRect();
          if (event.clientX >= rect.right - MENU_ACTION_WIDTH) onMenuOpenChange(!isMenuOpen);
          else {
            onToggle?.();
            onMenuOpenChange(false);
          }
        }}
        onPointerEnter={(event) => event.pointerType === "mouse" && sounds.hover()}
      >
        <Icon />
        <ChevronDown className={isMenuOpen ? "microphone-button__arrow is-open" : "microphone-button__arrow"} />
      </button>
      {isMenuOpen && (
        <div className="audio-dropdown microphone-dropdown" role="menu">
          <span className="audio-dropdown__arrow" />
          <div className="deafen-volume">
            <Mic aria-hidden="true" />
            <div className="deafen-volume__body">
              <div>
                <span>Input volume</span>
                <output>{inputVolume}%</output>
              </div>
              <input
                type="range"
                min={0}
                max={100}
                value={inputVolume}
                aria-label="Input volume"
                onChange={(event) => setPreference("inputVolume", Number(event.target.value))}
              />
            </div>
          </div>
          <AudioDeviceMenu kind="input" open={isMenuOpen} onSelect={() => onMenuOpenChange(false)} />
        </div>
      )}
    </div>
  );
}