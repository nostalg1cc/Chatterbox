import { CircleAlert } from "lucide-react";
import { useUiSounds } from "../hooks/useUiSounds";

export function AlertBar({ message }) {
  const uiSounds = useUiSounds();

  function handlePointerEnter(event) {
    if (event.pointerType === "mouse") {
      uiSounds.hover();
    }
  }

  return (
    <button
      type="button"
      className="status-bar"
      aria-label={message}
      onClick={uiSounds.click}
      onPointerEnter={handlePointerEnter}
    >
      <CircleAlert aria-hidden="true" strokeWidth={2} />
      <span>{message}</span>
    </button>
  );
}
