export function TypingIndicator({ name = "Partner", avatar = null }) {
  return (
    <div className="typing-indicator" role="status" aria-live="polite">
      <span className="typing-indicator__avatar" aria-hidden="true">
        {avatar ? <img src={avatar} alt="" /> : <span className="typing-indicator__fallback">{name.slice(0, 1).toUpperCase()}</span>}
      </span>
      <span className="typing-indicator__name">{name}</span>
      <span className="typing-indicator__label">is typing</span>
      <span className="typing-indicator__dots" aria-label="...">
        <i />
        <i />
        <i />
      </span>
    </div>
  );
}
