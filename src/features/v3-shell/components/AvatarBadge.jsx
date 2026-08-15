export function AvatarBadge() {
  return (
    <button
      type="button"
      className="avatar-badge"
      aria-label="nrohde profile"
    >
      <span className="avatar-badge__avatar" aria-hidden="true">
        <img className="avatar-badge__image" src="/iterated-button.jpg" alt="" />
      </span>
      <span className="avatar-badge__name">nrohde</span>
    </button>
  );
}
