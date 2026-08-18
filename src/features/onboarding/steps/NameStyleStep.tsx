import { DecoratedText, type TextDecoration } from "@/components/decorated-text";
import { NAME_COLOR_OPTIONS, nameColorClass } from "@/lib/name-colors";
import type { NameColor, NameFont, NameWeight } from "@/lib/types";

const NAME_FONTS: NameFont[] = ["sans", "rounded", "serif", "mono"];
const NAME_WEIGHTS: NameWeight[] = ["regular", "medium", "bold", "black"];
const NAME_EFFECTS = ["fuzzy", "sparkles", "resize", "bouncy", "wavy", "gradient", "glitch", "particle"];

export function NameStyleStep({
  displayName,
  nameFont,
  nameWeight,
  nameColor,
  nameDecoration,
  onFontChange,
  onWeightChange,
  onColorChange,
  onDecorationChange,
}: {
  displayName: string;
  nameFont: NameFont;
  nameWeight: NameWeight;
  nameColor: NameColor;
  nameDecoration: string | null;
  onFontChange: (font: NameFont) => void;
  onWeightChange: (weight: NameWeight) => void;
  onColorChange: (color: NameColor) => void;
  onDecorationChange: (effect: string | null) => void;
}) {
  return (
    <>
      <div className="v3-onboarding__name-preview">
        <p className={nameColorClass(nameColor)}>
          <DecoratedText effect={nameDecoration as TextDecoration | null} font={nameFont} weight={nameWeight}>
            {displayName.trim() || "Your display name"}
          </DecoratedText>
        </p>
      </div>

      <div className="v3-settings__panel">
        <div className="v3-settings__panel-section">
          <p className="v3-settings__field-label">Font</p>
          <div className="v3-settings__choice-group">
            {NAME_FONTS.map((font) => (
              <button key={font} type="button" className={"v3-settings__choice" + (nameFont === font ? " is-active" : "")} onClick={() => onFontChange(font)}>
                {font}
              </button>
            ))}
          </div>
        </div>
        <div className="v3-settings__panel-section">
          <p className="v3-settings__field-label">Weight</p>
          <div className="v3-settings__choice-group">
            {NAME_WEIGHTS.map((weight) => (
              <button key={weight} type="button" className={"v3-settings__choice" + (nameWeight === weight ? " is-active" : "")} onClick={() => onWeightChange(weight)}>
                {weight}
              </button>
            ))}
          </div>
        </div>
        <div className="v3-settings__panel-section">
          <p className="v3-settings__field-label">Effect</p>
          <div className="v3-settings__choice-group">
            {NAME_EFFECTS.map((effect) => (
              <button key={effect} type="button" className={"v3-settings__choice" + (nameDecoration === effect ? " is-active" : "")} onClick={() => onDecorationChange(effect)}>
                {effect}
              </button>
            ))}
            <button type="button" className={"v3-settings__choice" + (nameDecoration === null ? " is-active" : "")} onClick={() => onDecorationChange(null)}>
              None
            </button>
          </div>
        </div>
        <div className="v3-settings__panel-section">
          <p className="v3-settings__field-label">Color</p>
          <div className="v3-settings__swatch-grid">
            {NAME_COLOR_OPTIONS.map((option) => (
              <button
                key={option.value}
                type="button"
                aria-label={option.label}
                aria-pressed={nameColor === option.value}
                className={"v3-settings__swatch" + (nameColor === option.value ? " is-active" : "")}
                onClick={() => onColorChange(option.value)}
              >
                <span className={"v3-settings__swatch-dot " + option.swatch} />
              </button>
            ))}
          </div>
        </div>
      </div>
    </>
  );
}
