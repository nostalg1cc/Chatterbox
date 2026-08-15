import { CircleAlert } from "lucide-react";

export function AlertBar({ message }) {
  return (
    <button
      type="button"
      className="status-bar"
      aria-label={message}
    >
      <CircleAlert aria-hidden="true" strokeWidth={2} />
      <span>{message}</span>
    </button>
  );
}
