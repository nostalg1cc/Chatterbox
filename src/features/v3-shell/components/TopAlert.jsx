import { useEffect, useRef, useState } from "react";

export function TopAlert({ id, message, severity, icon: Icon, actions, progress, onDismiss, onVisibleChange }) {
  const [visible, setVisible] = useState(false);
  const shownRef = useRef(false);

  useEffect(() => {
    const enterFrame = window.requestAnimationFrame(() => setVisible(true));
    // Purely informational banners (no actions and no progress bar to wait
    // on) dismiss themselves; ones with actions, or a progress bar in
    // flight, stay up until the user picks an action or the progress
    // reaches its own conclusion.
    const autoDismiss = actions?.length || typeof progress === "number"
      ? null
      : window.setTimeout(() => setVisible(false), 5000);
    return () => {
      window.cancelAnimationFrame(enterFrame);
      if (autoDismiss) window.clearTimeout(autoDismiss);
    };
    // progress deliberately excluded - it can tick many times a second
    // during a download, and actions already changes (to []) at the exact
    // moment progress mode begins, which is the only transition that
    // matters here.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id, actions]);

  // Reported up so the parent can push/restore the surrounding layout in
  // lockstep with this banner's own slide, instead of waiting for the
  // delayed onDismiss below (which only fires once the exit transition has
  // finished, well after the banner is already gone).
  useEffect(() => {
    onVisibleChange?.(visible);
  }, [visible, onVisibleChange]);

  useEffect(() => {
    if (visible) {
      shownRef.current = true;
      return;
    }
    if (!shownRef.current) return; // still on the very first (hidden) render
    const timer = window.setTimeout(() => onDismiss(id), 340);
    return () => window.clearTimeout(timer);
  }, [visible, id, onDismiss]);

  const className = ["top-alert", `top-alert--${severity}`, visible && "top-alert--visible"]
    .filter(Boolean)
    .join(" ");

  return (
    <div className={className} role="status">
      {typeof progress === "number" && (
        <div className="top-alert__progress-fill" style={{ width: `${Math.max(0, Math.min(100, progress))}%` }} />
      )}
      <Icon className="top-alert__icon" aria-hidden="true" strokeWidth={2} />
      <span className="top-alert__message">{message}</span>
      {actions?.length > 0 && (
        <div className="top-alert__actions">
          {actions.map((action) => (
            <button
              key={action.label}
              type="button"
              className={"top-alert__action" + (action.confirm ? " top-alert__action--confirm" : "")}
              onClick={() => {
                action.onClick?.();
                if (!action.keepOpen) setVisible(false);
              }}
            >
              {action.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
