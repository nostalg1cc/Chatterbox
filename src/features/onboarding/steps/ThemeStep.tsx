import { CheckIcon } from "lucide-react";
import { THEME_OPTIONS } from "@/lib/themes";
import type { AppTheme } from "@/stores/preferences";

export function ThemeStep({ theme, onChange }: { theme: AppTheme; onChange: (theme: AppTheme) => void }) {
  return (
    <div className="v3-onboarding__step-heading">
      <h1>Pick a look</h1>
      <p>Sets the color palette for backgrounds, buttons, and other surfaces. Change it any time in Settings.</p>
      <div className="v3-settings__theme-grid" style={{ marginTop: 20 }}>
        {THEME_OPTIONS.map((option) => (
          <button
            key={option.value}
            type="button"
            aria-label={option.label}
            aria-pressed={theme === option.value}
            className={"v3-settings__theme-card" + (theme === option.value ? " is-active" : "")}
            onClick={() => onChange(option.value)}
          >
            <div className="v3-settings__theme-preview" style={{ background: option.bg }}>
              <span className="v3-settings__theme-preview-dot" style={{ background: option.material }} />
              <span className="v3-settings__theme-preview-bar" style={{ background: option.material }} />
            </div>
            <div className="v3-settings__theme-card-footer">
              <span>{option.label}</span>
              {theme === option.value && <CheckIcon aria-hidden="true" />}
            </div>
          </button>
        ))}
      </div>
    </div>
  );
}
