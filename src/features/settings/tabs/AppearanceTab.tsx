import { Slider } from "@/features/v3-shell/components/Slider";
import { Toggle } from "@/features/v3-shell/components/Toggle";
import { ThemePicker } from "../components/ThemePicker";
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

      <ThemePicker theme={preferences.theme} onChange={(value) => setPreference("theme", value)} />

      <div className="v3-settings__panel">
        <div className="v3-settings__panel-section">
          <p className="v3-settings__row-title" style={{ marginBottom: 4 }}>Background grain</p>
          <p className="v3-settings__row-desc" style={{ marginBottom: 14 }}>
            The subtle noise texture over the app background - keeps it from reading as dead-flat.
          </p>
          <Slider label="Grain size" value={preferences.grainSize} onChange={(value) => setPreference("grainSize", value)} />
        </div>
        <div className="v3-settings__panel-section">
          <Slider label="Grain intensity" value={preferences.grainIntensity} onChange={(value) => setPreference("grainIntensity", value)} />
        </div>
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
