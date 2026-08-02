import { useUiSounds } from "../hooks/useUiSounds";

export function AvatarBadge() {
  const uiSounds = useUiSounds();

  function handlePointerEnter(event) {
    if (event.pointerType === "mouse") {
      uiSounds.hover();
    }
  }

  return (
    <button
      type="button"
      className="avatar-badge"
      aria-label="nrohde profile"
      onClick={uiSounds.click}
      onPointerEnter={handlePointerEnter}
    >
      <span className="avatar-badge__avatar" aria-hidden="true">
        <img className="avatar-badge__image" src="/iterated-button.jpg" alt="" />
      </span>
      <span className="avatar-badge__name">nrohde</span>
    </button>
  );
}
