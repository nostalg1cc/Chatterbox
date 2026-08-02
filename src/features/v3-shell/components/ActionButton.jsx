import { useUiSounds } from "../hooks/useUiSounds";

export function ActionButton({
  icon: Icon,
  image,
  label,
  text,
  className: additionalClassName,
  children,
  type = "button",
  onClick,
  onPointerEnter,
  ...props
}) {
  const uiSounds = useUiSounds();
  const className = [
    "icon-button",
    image && "icon-button--image",
    text && "icon-button--label",
    additionalClassName,
  ]
    .filter(Boolean)
    .join(" ");

  function handleClick(event) {
    uiSounds.click();
    onClick?.(event);
  }

  function handlePointerEnter(event) {
    if (event.pointerType === "mouse") {
      uiSounds.hover();
    }

    onPointerEnter?.(event);
  }

  return (
    <button
      type={type}
      aria-label={label}
      className={className}
      onClick={handleClick}
      onPointerEnter={handlePointerEnter}
      {...props}
    >
      {children ?? (
        <>
          {image ? (
            <img
              className="icon-button__image"
              src={image}
              alt=""
              aria-hidden="true"
            />
          ) : (
            Icon && <Icon aria-hidden="true" strokeWidth={2} />
          )}
          {text && <span className="icon-button__label">{text}</span>}
        </>
      )}
    </button>
  );
}
