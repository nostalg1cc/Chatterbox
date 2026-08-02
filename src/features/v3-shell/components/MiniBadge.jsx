import { useUiSounds } from "../hooks/useUiSounds";

export function MiniBadge({ icon: Icon, label, onClick, playClickSound = true }) {
  const uiSounds = useUiSounds();

  function handleClick(event) {
    if (playClickSound) {
      uiSounds.click();
    }

    onClick?.(event);
  }

  function handlePointerEnter(event) {
    if (event.pointerType === "mouse") {
      uiSounds.hover();
    }
  }

  return (
    <button
      type="button"
      className="mini-badge"
      aria-label={label}
      onClick={handleClick}
      onPointerEnter={handlePointerEnter}
    >
      <Icon aria-hidden="true" strokeWidth={2} />
    </button>
  );
}
