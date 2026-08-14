import { useEffect, useRef, useState } from "react";

export function TopAlert({ id, message, severity, icon: Icon, actions, onDismiss }) {
  const [visible, setVisible] = useState(false);
  const shownRef = useRef(false);

  useEffect(() => {
    const enterFrame = window.requestAnimationFrame(() => setVisible(true));
    // Purely informational banners (no actions to wait on) dismiss
    // themselves; ones with actions stay up until the user picks one.
    const autoDismiss = actions?.length ? null : window.setTimeout(() => setVisible(false), 5000);
    return () => {
      window.cancelAnimationFrame(enterFrame);
      if (autoDismiss) window.clearTimeout(autoDismiss);
    };
  }, [id, actions]);

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
                setVisible(false);
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
