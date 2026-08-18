import { CheckIcon } from "lucide-react";
import { themesByFamily } from "@/lib/themes";
import type { AppTheme } from "@/stores/preferences";

// Shared by the Appearance tab and the onboarding theme step - grouped by
// color family (see themesByFamily) so each group reads as a shade
// progression (e.g. Pink & Red running pink -> magenta -> deep red) rather
// than one flat grid of unrelated hues.
export function ThemePicker({ theme, onChange }: { theme: AppTheme; onChange: (theme: AppTheme) => void }) {
  return (
    <>
      {themesByFamily().map(([family, options]) => (
        <div key={family} className="v3-settings__theme-family">
          <p className="v3-settings__section-label">{family}</p>
          <div className="v3-settings__theme-row">
            {options.map((option) => (
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
      ))}
    </>
  );
}
