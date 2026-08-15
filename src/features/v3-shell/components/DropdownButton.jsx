import { ChevronDown } from "lucide-react";

export function DropdownButton({
  icon: Icon,
  label,
  isMenuOpen,
  onMenuOpenChange,
  children,
  className = "",
}) {
  function handleClick() {
    onMenuOpenChange(!isMenuOpen);
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
