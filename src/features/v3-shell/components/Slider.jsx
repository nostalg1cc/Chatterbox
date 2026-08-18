export function Slider({ label, value, min = 0, max = 100, formatValue, onChange }) {
  return (
    <div className="v3-slider">
      <div className="v3-slider__head">
        <span>{label}</span>
        <span className="v3-slider__value">{formatValue ? formatValue(value) : value + "%"}</span>
      </div>
      <input
        type="range"
        min={min}
        max={max}
        value={value}
        aria-label={label}
        className="v3-slider__input"
        onChange={(event) => onChange(Number(event.target.value))}
      />
    </div>
  );
}
