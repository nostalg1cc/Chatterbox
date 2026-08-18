import { useEffect, useRef, useState } from "react";
import { Loader2Icon, PlusIcon } from "lucide-react";
import { SoundRow } from "../components/SoundRow";
import { formattedBytes } from "@/lib/media";
import { useAlerts } from "@/stores/alerts";
import { useSoundboard } from "@/stores/soundboard";

const SOUND_STORAGE_LIMIT = 16 * 1024 * 1024;

export function SoundboardTab() {
  const sounds = useSoundboard((state) => state.sounds);
  const uploading = useSoundboard((state) => state.uploading);
  const soundInput = useRef<HTMLInputElement>(null);
  const [soundName, setSoundName] = useState("");
  const storageBytes = sounds.reduce((total, sound) => total + sound.size_bytes, 0);

  useEffect(() => {
    void useSoundboard.getState().load();
  }, []);

  const uploadSound = async (file: File | undefined) => {
    if (!file) return;
    const name = soundName.trim() || file.name.replace(/\.[^.]+$/, "").slice(0, 32);
    try {
      await useSoundboard.getState().upload(file, name);
      setSoundName("");
    } catch (error) {
      useAlerts.getState().show({ severity: "danger", message: error instanceof Error ? error.message : "Couldn't add the sound." });
    } finally {
      if (soundInput.current) soundInput.current.value = "";
    }
  };

  return (
    <div className="v3-settings__tab-panel">
      <div className="v3-settings__heading">
        <h2>Soundboard</h2>
        <p>Clips are trimmed, normalized, and converted locally to 48 kHz mono Opus at 96 kbps.</p>
      </div>

      <div className="v3-settings__panel">
        <div className="v3-settings__panel-section">
          <div className="v3-settings__storage-head">
            <span>Sound storage</span>
            <span className="v3-settings__storage-value">{formattedBytes(storageBytes)} / 16 MiB</span>
          </div>
          <div className="v3-settings__storage-bar">
            <div
              className={"v3-settings__storage-bar-fill" + (storageBytes / SOUND_STORAGE_LIMIT > 0.9 ? " is-near-limit" : "")}
              style={{ width: Math.min(100, (storageBytes / SOUND_STORAGE_LIMIT) * 100) + "%" }}
            />
          </div>
          <p className="v3-settings__row-desc" style={{ marginTop: 8 }}>
            Storage is the limit - add as many sounds as fit within your allowance.
          </p>
          <div className="v3-settings__sound-add">
            <input
              className="v3-settings__input"
              value={soundName}
              maxLength={32}
              placeholder="Optional sound name"
              onChange={(event) => setSoundName(event.target.value)}
            />
            <input ref={soundInput} className="v3-settings__hidden-input" type="file" accept="audio/*" onChange={(event) => void uploadSound(event.target.files?.[0])} />
            <button
              type="button"
              className="v3-settings__ghost-button"
              disabled={uploading || storageBytes >= SOUND_STORAGE_LIMIT}
              onClick={() => soundInput.current?.click()}
            >
              {uploading ? <Loader2Icon aria-hidden="true" /> : <PlusIcon aria-hidden="true" />}
              Add sound
            </button>
          </div>
          <p className="v3-settings__row-desc" style={{ marginTop: 8 }}>15 seconds max / 512 KiB prepared max per sound.</p>
        </div>
      </div>

      {sounds.length > 0 ? (
        <div className="v3-settings__panel">
          {sounds.map((sound) => (
            <SoundRow key={sound.id} sound={sound} />
          ))}
        </div>
      ) : (
        <p className="v3-settings__empty-state">No sounds yet. Add a short clip, then play it from the chat header while in voice.</p>
      )}
    </div>
  );
}
