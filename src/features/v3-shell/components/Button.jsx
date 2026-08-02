export function Button({ children, type = "button", ...props }) {
  return (
    <button className="glass-button" type={type} {...props}>
      <span className="glass-button__label">{children}</span>
    </button>
  );
}
