import { useEffect, useRef, useState } from "react";
import { Loader2Icon, PauseIcon, PlayIcon, PlusIcon, ScissorsIcon, XIcon } from "lucide-react";
import { SoundRow } from "../components/SoundRow";
import { formattedBytes } from "@/lib/media";
import { useAlerts } from "@/stores/alerts";
import { useSoundboard } from "@/stores/soundboard";

const SOUND_STORAGE_LIMIT = 16 * 1024 * 1024;
const MAX_SOURCE_BYTES = 25 * 1024 * 1024;
const MAX_DURATION_MS = 15_000;
const MIN_DURATION_MS = 100;

function formatSoundTime(milliseconds: number) {
  const totalSeconds = Math.max(0, milliseconds) / 1000;
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds - minutes * 60;
  return `${minutes}:${seconds.toFixed(1).padStart(4, "0")}`;
}

async function readAudioDuration(file: File) {
  const url = URL.createObjectURL(file);
  try {
    const duration = await new Promise<number>((resolve, reject) => {
      const audio = new Audio();
      audio.preload = "metadata";
      audio.onloadedmetadata = () => resolve(audio.duration);
      audio.onerror = () => reject(new Error("This audio format could not be read."));
      audio.src = url;
    });
    if (!Number.isFinite(duration) || duration * 1000 < MIN_DURATION_MS) throw new Error("The sound is too short.");
    return { durationMs: Math.round(duration * 1000), url };
  } catch (error) {
    URL.revokeObjectURL(url);
    throw error;
  }
}

export function SoundboardTab() {
  const sounds = useSoundboard((state) => state.sounds);
  const uploading = useSoundboard((state) => state.uploading);
  const soundInput = useRef<HTMLInputElement>(null);
  const previewAudio = useRef<HTMLAudioElement>(null);
  const [draftFile, setDraftFile] = useState<File | null>(null);
  const [draftUrl, setDraftUrl] = useState<string | null>(null);
  const [soundName, setSoundName] = useState("");
  const [sourceDurationMs, setSourceDurationMs] = useState(0);
  const [trimStartMs, setTrimStartMs] = useState(0);
  const [trimEndMs, setTrimEndMs] = useState(0);
  const [previewing, setPreviewing] = useState(false);
  const storageBytes = sounds.reduce((total, sound) => total + sound.size_bytes, 0);
  const selectedDurationMs = Math.max(0, trimEndMs - trimStartMs);
  const selectionIsValid = selectedDurationMs >= MIN_DURATION_MS && selectedDurationMs <= MAX_DURATION_MS;

  useEffect(() => {
    void useSoundboard.getState().load();
  }, []);

  useEffect(() => () => {
    if (draftUrl) URL.revokeObjectURL(draftUrl);
  }, [draftUrl]);

  const stopPreview = () => {
    const audio = previewAudio.current;
    if (audio) {
      audio.pause();
      audio.currentTime = trimStartMs / 1000;
    }
    setPreviewing(false);
  };

  const clearDraft = () => {
    stopPreview();
    setDraftFile(null);
    setDraftUrl(null);
    setSoundName("");
    setSourceDurationMs(0);
    setTrimStartMs(0);
    setTrimEndMs(0);
    if (soundInput.current) soundInput.current.value = "";
  };

  const chooseSound = async (file: File | undefined) => {
    if (!file) return;
    if (!file.type.startsWith("audio/")) {
      useAlerts.getState().show({ severity: "danger", message: "Choose an audio file." });
      return;
    }
    if (file.size > MAX_SOURCE_BYTES) {
      useAlerts.getState().show({ severity: "danger", message: "Source audio can be up to 25 MiB." });
      return;
    }
    try {
      const { durationMs, url } = await readAudioDuration(file);
      stopPreview();
      setDraftFile(file);
      setDraftUrl(url);
      setSoundName(file.name.replace(/\.[^.]+$/, "").slice(0, 32));
      setSourceDurationMs(durationMs);
      setTrimStartMs(0);
      setTrimEndMs(Math.min(durationMs, MAX_DURATION_MS));
    } catch (error) {
      useAlerts.getState().show({ severity: "danger", message: error instanceof Error ? error.message : "This audio file could not be read." });
    } finally {
      if (soundInput.current) soundInput.current.value = "";
    }
  };

  const previewSelection = async () => {
    const audio = previewAudio.current;
    if (!audio || !draftUrl) return;
    if (previewing) {
      stopPreview();
      return;
    }
    audio.currentTime = trimStartMs / 1000;
    try {
      await audio.play();
      setPreviewing(true);
    } catch {
      useAlerts.getState().show({ severity: "danger", message: "Preview could not play on this device." });
    }
  };

  const uploadSound = async () => {
    if (!draftFile) return;
    const name = soundName.trim();
    if (!name) {
      useAlerts.getState().show({ severity: "danger", message: "Give the sound a name before saving it." });
      return;
    }
    if (!selectionIsValid) {
      useAlerts.getState().show({ severity: "danger", message: "Choose a clip between 0.1 and 15 seconds." });
      return;
    }
    try {
      await useSoundboard.getState().upload(draftFile, name, { startMs: trimStartMs, endMs: trimEndMs });
      clearDraft();
    } catch (error) {
      useAlerts.getState().show({ severity: "danger", message: error instanceof Error ? error.message : "Couldn't add the sound." });
    }
  };

  const updateTrimStart = (value: number) => {
    stopPreview();
    setTrimStartMs(Math.min(value, trimEndMs - MIN_DURATION_MS));
  };

  const updateTrimEnd = (value: number) => {
    stopPreview();
    setTrimEndMs(Math.max(value, trimStartMs + MIN_DURATION_MS));
  };

  return (
    <div className="v3-settings__tab-panel">
      <div className="v3-settings__heading">
        <h2>Soundboard</h2>
        <p>Choose a moment first, trim it, name it, and preview the exact clip before it uses any storage.</p>
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
            Storage is the limit — add as many sounds as fit within your allowance.
          </p>

          <input
            ref={soundInput}
            className="v3-settings__hidden-input"
            type="file"
            accept="audio/*"
            onChange={(event) => void chooseSound(event.target.files?.[0])}
          />

          {!draftFile ? (
            <div className="v3-settings__sound-add">
              <button
                type="button"
                className="v3-settings__ghost-button"
                disabled={uploading || storageBytes >= SOUND_STORAGE_LIMIT}
                onClick={() => soundInput.current?.click()}
              >
                <PlusIcon aria-hidden="true" />
                Choose audio
              </button>
              <p className="v3-settings__row-desc">Source files up to 25 MiB. The saved clip is locally normalized and compressed to 48 kHz mono Opus.</p>
            </div>
          ) : (
            <div className="v3-settings__sound-draft">
              <audio
                ref={previewAudio}
                src={draftUrl ?? undefined}
                onEnded={() => setPreviewing(false)}
                onTimeUpdate={(event) => {
                  if (event.currentTarget.currentTime * 1000 >= trimEndMs) stopPreview();
                }}
              />
              <div className="v3-settings__sound-draft-head">
                <div>
                  <p className="v3-settings__row-title">{draftFile.name}</p>
                  <p className="v3-settings__row-desc">Original {formatSoundTime(sourceDurationMs)} · selected {formatSoundTime(selectedDurationMs)}</p>
                </div>
                <button type="button" className="v3-settings__icon-button" aria-label="Discard selected sound" onClick={clearDraft}>
                  <XIcon aria-hidden="true" />
                </button>
              </div>

              <div className="v3-settings__field">
                <label htmlFor="soundboard-draft-name" className="v3-settings__field-label">Sound name</label>
                <input
                  id="soundboard-draft-name"
                  className="v3-settings__input"
                  value={soundName}
                  maxLength={32}
                  placeholder="Name this sound"
                  onChange={(event) => setSoundName(event.target.value)}
                />
              </div>

              <div className="v3-settings__sound-trim-head">
                <span><ScissorsIcon aria-hidden="true" /> Trim clip</span>
                <span className={selectionIsValid ? "" : "is-invalid"}>{selectionIsValid ? "Ready to save" : "15 seconds max"}</span>
              </div>
              <div className="v3-settings__sound-trim-grid">
                <label>
                  <span>Start · {formatSoundTime(trimStartMs)}</span>
                  <input
                    type="range"
                    min="0"
                    max={Math.max(0, trimEndMs - MIN_DURATION_MS)}
                    step="10"
                    value={trimStartMs}
                    onChange={(event) => updateTrimStart(Number(event.target.value))}
                  />
                </label>
                <label>
                  <span>End · {formatSoundTime(trimEndMs)}</span>
                  <input
                    type="range"
                    min={trimStartMs + MIN_DURATION_MS}
                    max={sourceDurationMs}
                    step="10"
                    value={trimEndMs}
                    onChange={(event) => updateTrimEnd(Number(event.target.value))}
                  />
                </label>
              </div>

              <div className="v3-settings__sound-draft-actions">
                <button type="button" className="v3-settings__ghost-button" onClick={() => void previewSelection()}>
                  {previewing ? <PauseIcon aria-hidden="true" /> : <PlayIcon aria-hidden="true" />}
                  {previewing ? "Stop preview" : "Preview selection"}
                </button>
                <button
                  type="button"
                  className="v3-settings__ghost-button is-primary"
                  disabled={uploading || !selectionIsValid || !soundName.trim() || storageBytes >= SOUND_STORAGE_LIMIT}
                  onClick={() => void uploadSound()}
                >
                  {uploading ? <Loader2Icon aria-hidden="true" /> : <PlusIcon aria-hidden="true" />}
                  {uploading ? "Saving sound…" : "Add to soundboard"}
                </button>
              </div>
              <p className="v3-settings__row-desc">The final clip is normalized, encoded at 96 kbps, and must fit inside 512 KiB.</p>
            </div>
          )}
        </div>
      </div>

      {sounds.length > 0 ? (
        <div className="v3-settings__panel">
          {sounds.map((sound) => (
            <SoundRow key={sound.id} sound={sound} />
          ))}
        </div>
      ) : (
        <p className="v3-settings__empty-state">No sounds yet. Choose a short clip, make it yours, then add it to use in voice.</p>
      )}
    </div>
  );
}
