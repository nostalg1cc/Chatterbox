import { useState } from "react";
import { CheckIcon, Music2Icon, PencilIcon, PlayIcon, Trash2Icon, XIcon } from "lucide-react";
import { formattedBytes } from "@/lib/media";
import { useAlerts } from "@/stores/alerts";
import { useSoundboard, type SoundboardSound } from "@/stores/soundboard";

export function SoundRow({ sound }: { sound: SoundboardSound }) {
  const [renaming, setRenaming] = useState(false);
  const [name, setName] = useState(sound.name);

  const rename = async () => {
    try {
      await useSoundboard.getState().rename(sound.id, name);
      setRenaming(false);
    } catch (error) {
      useAlerts.getState().show({ severity: "danger", message: error instanceof Error ? error.message : "Couldn't rename the sound." });
    }
  };

  return (
    <div className="v3-settings__sound-row">
      <Music2Icon aria-hidden="true" className="v3-settings__row-icon" />
      <div className="v3-settings__row-copy">
        {renaming ? (
          <input
            autoFocus
            value={name}
            maxLength={32}
            aria-label="Sound name"
            className="v3-settings__input v3-settings__input--inline"
            onChange={(event) => setName(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") void rename();
              if (event.key === "Escape") setRenaming(false);
            }}
          />
        ) : (
          <p className="v3-settings__row-title">{sound.name}</p>
        )}
        <p className="v3-settings__row-desc">{(sound.duration_ms / 1000).toFixed(1)}s / {formattedBytes(sound.size_bytes)}</p>
      </div>
      {renaming ? (
        <>
          <button type="button" className="v3-settings__icon-button" aria-label="Save sound name" onClick={() => void rename()}>
            <CheckIcon aria-hidden="true" />
          </button>
          <button type="button" className="v3-settings__icon-button" aria-label="Cancel rename" onClick={() => setRenaming(false)}>
            <XIcon aria-hidden="true" />
          </button>
        </>
      ) : (
        <>
          <button type="button" className="v3-settings__icon-button" aria-label={"Preview " + sound.name} onClick={() => void useSoundboard.getState().preview(sound.id)}>
            <PlayIcon aria-hidden="true" />
          </button>
          <button type="button" className="v3-settings__icon-button" aria-label={"Rename " + sound.name} onClick={() => { setName(sound.name); setRenaming(true); }}>
            <PencilIcon aria-hidden="true" />
          </button>
        </>
      )}
      <button type="button" className="v3-settings__icon-button is-danger" aria-label={"Delete " + sound.name} onClick={() => void useSoundboard.getState().remove(sound.id)}>
        <Trash2Icon aria-hidden="true" />
      </button>
    </div>
  );
}
