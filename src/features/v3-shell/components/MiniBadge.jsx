export function MiniBadge({ icon: Icon, label, onClick }) {
  return (
    <button
      type="button"
      className="mini-badge"
      aria-label={label}
      onClick={onClick}
    >
      <Icon aria-hidden="true" strokeWidth={2} />
    </button>
  );
}
