import { Paperclip } from "lucide-react";

export function InputBar({ value, onChange, onSubmit, onPaste, onKeyDown, onAttach, commandName = null }) {
  function handleSubmit(event) { event.preventDefault(); onSubmit?.(); }
  return (
    <form
      className={"text-input-bar" + (onAttach ? " text-input-bar--with-attach" : "") + (commandName ? " text-input-bar--command" : "")}
      onSubmit={handleSubmit}
    >
      <label className="text-input-bar__label" htmlFor="message-composer">Write something</label>
      {onAttach && (
        <button type="button" className="text-input-bar__attach" aria-label="Attach media" onClick={onAttach}>
          <Paperclip aria-hidden="true" />
        </button>
      )}
      {commandName && <span className="text-input-bar__command-chip">/{commandName}</span>}
      <input
        id="message-composer"
        type="text"
        value={value}
        onChange={(event) => onChange?.(event.target.value)}
        onPaste={onPaste}
        onKeyDown={onKeyDown}
        placeholder={commandName ? "Add a message (optional)…" : "Write something..."}
        aria-label={commandName ? `/${commandName} argument` : "Write something"}
        autoComplete="off"
      />
    </form>
  );
}
