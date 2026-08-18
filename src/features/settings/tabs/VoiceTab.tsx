import { useEffect, useState } from "react";
import { MicIcon, Volume2Icon } from "lucide-react";
import { Slider } from "@/features/v3-shell/components/Slider";
import { Toggle } from "@/features/v3-shell/components/Toggle";
import { DeviceSelect } from "../components/DeviceSelect";
import { MicrophoneTest } from "../components/MicrophoneTest";
import { playAppSound } from "@/lib/app-sounds";
import { isTauri } from "@/lib/tauri";
import { usePreferences } from "@/stores/preferences";

export function VoiceTab() {
  const preferences = usePreferences();
  const setPreference = usePreferences((state) => state.setPreference);
  const [devices, setDevices] = useState<MediaDeviceInfo[]>([]);

  useEffect(() => {
    if (!navigator.mediaDevices?.enumerateDevices) return;
    void navigator.mediaDevices.enumerateDevices().then(setDevices).catch(() => setDevices([]));
  }, []);

  const microphones = devices.filter((device) => device.kind === "audioinput");
  const speakers = devices.filter((device) => device.kind === "audiooutput");

  return (
    <div className="v3-settings__tab-panel">
      <div className="v3-settings__heading">
        <h2>Voice & Video</h2>
        <p>Device changes apply immediately while you are connected to voice.</p>
      </div>

      <p className="v3-settings__section-label">Microphone</p>
      <div className="v3-settings__panel">
        <div className="v3-settings__panel-section">
          <DeviceSelect
            id="settings-audio-input"
            label="Input device"
            icon={<MicIcon aria-hidden="true" />}
            value={preferences.inputDeviceId}
            devices={microphones}
            fallback="Default microphone"
            onChange={(value) => setPreference("inputDeviceId", value)}
          />
        </div>
        <div className="v3-settings__panel-section">
          <Slider label="Input volume" value={preferences.inputVolume} onChange={(value) => setPreference("inputVolume", value)} />
        </div>
        <div className="v3-settings__panel-section">
          <div className="v3-settings__row v3-settings__row--tight">
            <div className="v3-settings__row-copy">
              <p className="v3-settings__row-title">Noise suppression</p>
              <p className="v3-settings__row-desc">Off by default. Echo cancellation and automatic gain are always disabled.</p>
            </div>
            <Toggle checked={preferences.noiseSuppression} onChange={(value) => setPreference("noiseSuppression", value)} label="Noise suppression" />
          </div>
          {preferences.noiseSuppression && (
            <div className="v3-settings__engine-row">
              <span>Engine</span>
              <button type="button" className="v3-settings__choice is-active">WebRTC native</button>
              <button type="button" className="v3-settings__choice" disabled>RNNoise (soon)</button>
            </div>
          )}
        </div>
        <div className="v3-settings__panel-section">
          <MicrophoneTest
            inputDeviceId={preferences.inputDeviceId}
            inputVolume={preferences.inputVolume}
            noiseSuppression={preferences.noiseSuppression}
            outputDeviceId={preferences.outputDeviceId}
            outputVolume={preferences.outputVolume}
          />
        </div>
      </div>

      <p className="v3-settings__section-label">Speaker</p>
      <div className="v3-settings__panel">
        <div className="v3-settings__panel-section">
          <DeviceSelect
            id="settings-audio-output"
            label="Output device"
            icon={<Volume2Icon aria-hidden="true" />}
            value={preferences.outputDeviceId}
            devices={speakers}
            fallback="Default speakers"
            onChange={(value) => setPreference("outputDeviceId", value)}
          />
        </div>
        <div className="v3-settings__panel-section">
          <Slider label="Output volume" value={preferences.outputVolume} onChange={(value) => setPreference("outputVolume", value)} />
        </div>
        <div className="v3-settings__panel-section">
          <Slider
            label="Partner voice boost"
            value={preferences.partnerVoiceBoost}
            min={100}
            max={200}
            onChange={(value) => setPreference("partnerVoiceBoost", value)}
          />
          <p className="v3-settings__row-desc" style={{ marginTop: 8 }}>
            Local only. 100% is neutral; up to 200% boosts incoming voice without changing what your partner hears.
          </p>
        </div>
        <div className="v3-settings__panel-section">
          <div className="v3-settings__row v3-settings__row--tight">
            <div className="v3-settings__row-copy">
              <p className="v3-settings__row-title">Output test</p>
              <p className="v3-settings__row-desc">Plays through the selected device, with Windows default as fallback.</p>
            </div>
            <button type="button" className="v3-settings__ghost-button" onClick={() => playAppSound("notification_single", true)}>
              Test sound
            </button>
          </div>
        </div>
      </div>

      {isTauri && (
        <>
          <p className="v3-settings__section-label">Floating voice indicator</p>
          <div className="v3-settings__panel">
            <div className="v3-settings__panel-section">
              <p className="v3-settings__row-desc" style={{ marginBottom: 12 }}>
                A small always-on-top window showing who's talking, for when Nitro is minimized behind a game.
              </p>
              <Slider
                label="Overlay size"
                value={preferences.voiceHudScale}
                min={60}
                max={160}
                onChange={(value) => setPreference("voiceHudScale", value)}
              />
            </div>
            <div className="v3-settings__row">
              <div className="v3-settings__row-copy">
                <p className="v3-settings__row-title">Show names</p>
                <p className="v3-settings__row-desc">Display each participant's name next to their avatar in the overlay.</p>
              </div>
              <Toggle checked={preferences.voiceHudShowNames} onChange={(value) => setPreference("voiceHudShowNames", value)} label="Show names" />
            </div>
          </div>
        </>
      )}

      <p className="v3-settings__section-label">Sounds</p>
      <div className="v3-settings__panel">
        <div className="v3-settings__row">
          <div className="v3-settings__row-copy">
            <p className="v3-settings__row-title">Interface sounds</p>
            <p className="v3-settings__row-desc">Play join, leave, mute, and deafen feedback.</p>
          </div>
          <Toggle checked={preferences.interfaceSounds} onChange={(value) => setPreference("interfaceSounds", value)} label="Interface sounds" />
        </div>
        <div className="v3-settings__panel-section">
          <Slider label="Interface sound volume" value={preferences.interfaceSoundVolume} onChange={(value) => setPreference("interfaceSoundVolume", value)} />
        </div>
        <div className="v3-settings__panel-section">
          <Slider label="Your soundboard volume" value={preferences.soundboardVolume} onChange={(value) => setPreference("soundboardVolume", value)} />
        </div>
      </div>

      <p className="v3-settings__footnote">
        Microphone permission is requested only when you join voice. Native suppression uses WebRTC's microphone
        constraint; echo cancellation and automatic gain stay off. Device names appear after the first join;
        unsupported output routing uses Windows default.
      </p>
    </div>
  );
}
