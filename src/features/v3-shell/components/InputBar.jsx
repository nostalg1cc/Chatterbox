import { Paperclip } from "lucide-react";

export function InputBar({ value, onChange, onSubmit, onPaste, onKeyDown, onAttach }) {
  function handleSubmit(event) { event.preventDefault(); onSubmit?.(); }
  return (
    <form className={"text-input-bar" + (onAttach ? " text-input-bar--with-attach" : "")} onSubmit={handleSubmit}>
      <label className="text-input-bar__label" htmlFor="message-composer">Write something</label>
      {onAttach && (
        <button type="button" className="text-input-bar__attach" aria-label="Attach media" onClick={onAttach}>
          <Paperclip aria-hidden="true" />
        </button>
      )}
      <input id="message-composer" type="text" value={value} onChange={(event) => onChange?.(event.target.value)} onPaste={onPaste} onKeyDown={onKeyDown} placeholder="Write something..." aria-label="Write something" autoComplete="off" />
    </form>
  );
}
