import { ChevronDown } from "lucide-react";
import { useUiSounds } from "../hooks/useUiSounds";

export function DropdownButton({
  icon: Icon,
  label,
  isMenuOpen,
  onMenuOpenChange,
  children,
  className = "",
}) {
  const uiSounds = useUiSounds();

  function handleClick() {
    onMenuOpenChange(!isMenuOpen);
  }

  function handlePointerEnter(event) {
    if (event.pointerType === "mouse") {
      uiSounds.hover();
    }
  }

  return (
    <div className={`audio-control ${className}`.trim()}>
      <button
        type="button"
        aria-label={label}
        aria-haspopup="menu"
        aria-expanded={isMenuOpen}
        className="icon-button dropdown-button"
        onClick={handleClick}
        onPointerEnter={handlePointerEnter}
      >
        <Icon aria-hidden="true" strokeWidth={2} />
        <ChevronDown
          className={isMenuOpen ? "dropdown-button__arrow is-open" : "dropdown-button__arrow"}
          aria-hidden="true"
          strokeWidth={2}
        />
      </button>
      {isMenuOpen && (
        <div className="audio-dropdown" aria-label={`${label} options`}>
          <span className="audio-dropdown__arrow" aria-hidden="true" />
          {children}
        </div>
      )}
    </div>
  );
}
