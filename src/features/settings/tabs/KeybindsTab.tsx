import { Toggle } from "@/features/v3-shell/components/Toggle";
import { KeybindRow } from "../components/KeybindRow";
import { usePreferences, type KeybindPreferences } from "@/stores/preferences";

export function KeybindsTab() {
  const globalVoiceShortcuts = usePreferences((state) => state.globalVoiceShortcuts);
  const keybinds = usePreferences((state) => state.keybinds);
  const setPreference = usePreferences((state) => state.setPreference);

  return (
    <div className="v3-settings__tab-panel">
      <div className="v3-settings__heading">
        <h2>Keybinds</h2>
        <p>Shortcuts work anywhere inside Nitro. Recording a duplicate replaces the old binding.</p>
      </div>

      <div className="v3-settings__panel">
        <div className="v3-settings__row">
          <div className="v3-settings__row-copy">
            <p className="v3-settings__row-title">Allow global mute & deafen</p>
            <p className="v3-settings__row-desc">
              Lets these two shortcuts work while Nitro is unfocused. Other shortcuts stay app-only.
            </p>
          </div>
          <Toggle
            checked={globalVoiceShortcuts}
            onChange={(value) => setPreference("globalVoiceShortcuts", value)}
            label="Allow global mute & deafen"
          />
        </div>
      </div>

      <div className="v3-settings__panel">
        {(Object.keys(keybinds) as (keyof KeybindPreferences)[]).map((key) => (
          <KeybindRow
            key={key}
            action={key}
            value={keybinds[key]}
            onChange={(value) => {
              const next = { ...keybinds };
              for (const existing of Object.keys(next) as (keyof KeybindPreferences)[]) {
                if (next[existing] === value) next[existing] = "";
              }
              next[key] = value;
              setPreference("keybinds", next);
            }}
          />
        ))}
      </div>
    </div>
  );
}
