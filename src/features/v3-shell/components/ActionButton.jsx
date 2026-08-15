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
  const className = [
    "icon-button",
    image && "icon-button--image",
    text && "icon-button--label",
    additionalClassName,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <button
      type={type}
      aria-label={label}
      className={className}
      onClick={onClick}
      onPointerEnter={onPointerEnter}
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
