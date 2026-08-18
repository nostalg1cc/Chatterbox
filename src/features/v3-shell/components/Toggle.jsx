export function Toggle({ checked, onChange, label }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      className={"v3-toggle" + (checked ? " is-on" : "")}
      onClick={() => onChange(!checked)}
    >
      <span className="v3-toggle__knob" />
    </button>
  );
}
