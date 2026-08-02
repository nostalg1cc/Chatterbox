export function InputBar({ value, onChange, onSubmit, onPaste, onKeyDown }) {
  function handleSubmit(event) { event.preventDefault(); onSubmit?.(); }
  return (
    <form className="text-input-bar" onSubmit={handleSubmit}>
      <label className="text-input-bar__label" htmlFor="message-composer">Write something</label>
      <input id="message-composer" type="text" value={value} onChange={(event) => onChange?.(event.target.value)} onPaste={onPaste} onKeyDown={onKeyDown} placeholder="Write something..." aria-label="Write something" autoComplete="off" />
    </form>
  );
}
