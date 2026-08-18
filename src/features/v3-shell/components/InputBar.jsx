import { useLayoutEffect, useRef } from "react";
import { Paperclip } from "lucide-react";

const MAX_HEIGHT_PX = 200;

export function InputBar({ value, onChange, onSubmit, onPaste, onKeyDown, onAttach, commandName = null }) {
  const textareaRef = useRef(null);

  // Auto-grow: measure natural content height with height reset to auto
  // first (otherwise scrollHeight would just report the previously-set
  // height back), then clamp to MAX_HEIGHT_PX and let the textarea's own
  // overflow-y:auto take over past that.
  useLayoutEffect(() => {
    const node = textareaRef.current;
    if (!node) return;
    node.style.height = "auto";
    node.style.height = Math.min(node.scrollHeight, MAX_HEIGHT_PX) + "px";
  }, [value]);

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
      <textarea
        ref={textareaRef}
        id="message-composer"
        rows={1}
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
