import { CheckIcon } from "lucide-react";
import { Toggle } from "@/features/v3-shell/components/Toggle";
import { THEME_OPTIONS } from "@/lib/themes";
import { isTauri } from "@/lib/tauri";
import { usePreferences } from "@/stores/preferences";

export function AppearanceTab() {
  const preferences = usePreferences();
  const setPreference = usePreferences((state) => state.setPreference);

  return (
    <div className="v3-settings__tab-panel">
      <div className="v3-settings__heading">
        <h2>Appearance</h2>
        <p>Choose the color palette for backgrounds, buttons, and other surfaces.</p>
      </div>
      <div className="v3-settings__theme-grid">
        {THEME_OPTIONS.map((option) => (
          <button
            key={option.value}
            type="button"
            aria-label={option.label}
            aria-pressed={preferences.theme === option.value}
            className={"v3-settings__theme-card" + (preferences.theme === option.value ? " is-active" : "")}
            onClick={() => setPreference("theme", option.value)}
          >
            <div className="v3-settings__theme-preview" style={{ background: option.bg }}>
              <span className="v3-settings__theme-preview-dot" style={{ background: option.material }} />
              <span className="v3-settings__theme-preview-bar" style={{ background: option.material }} />
            </div>
            <div className="v3-settings__theme-card-footer">
              <span>{option.label}</span>
              {preferences.theme === option.value && <CheckIcon aria-hidden="true" />}
            </div>
          </button>
        ))}
      </div>

      {isTauri && (
        <div className="v3-settings__panel">
          <div className="v3-settings__row">
            <div className="v3-settings__row-copy">
              <p className="v3-settings__row-title">Acrylic window material</p>
              <p className="v3-settings__row-desc">Off uses Windows 11 Mica; on uses Acrylic for the full app backdrop.</p>
            </div>
            <Toggle
              checked={preferences.windowMaterial === "acrylic"}
              onChange={(value) => setPreference("windowMaterial", value ? "acrylic" : "mica")}
              label="Acrylic window material"
            />
          </div>
          {preferences.windowMaterial === "acrylic" && (
            <div className="v3-settings__panel-section">
              <div className="v3-slider__head">
                <span>Acrylic dim</span>
                <span className="v3-slider__value">{preferences.acrylicDim}%</span>
              </div>
              <input
                type="range"
                min={0}
                max={100}
                value={preferences.acrylicDim}
                aria-label="Acrylic dim"
                className="v3-slider__input"
                onChange={(event) => setPreference("acrylicDim", Number(event.target.value))}
              />
              <p className="v3-settings__row-desc" style={{ marginTop: 4 }}>Adds a black tint across the window backdrop.</p>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
