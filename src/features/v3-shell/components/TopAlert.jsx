import { useEffect, useState } from "react";

export function TopAlert({ id, message, type, icon: Icon, isPrimary, onDismiss }) {
  const [phase, setPhase] = useState("enter");

  useEffect(() => {
    const entryFrame = window.requestAnimationFrame(() => setPhase("icon"));
    const expandTimer = window.setTimeout(() => setPhase("expanded"), 355);
    const collapseTimer = window.setTimeout(() => setPhase("collapsing"), 5000);
    const leaveTimer = window.setTimeout(() => setPhase("leaving"), 5280);
    const dismissTimer = window.setTimeout(() => onDismiss(id), 5620);

    return () => {
      window.cancelAnimationFrame(entryFrame);
      window.clearTimeout(expandTimer);
      window.clearTimeout(collapseTimer);
      window.clearTimeout(leaveTimer);
      window.clearTimeout(dismissTimer);
    };
  }, [id, onDismiss]);

  const className = ["top-alert", `top-alert--${type}`, `top-alert--${phase}`]
    .filter(Boolean)
    .join(" ");

  return (
    <div className={className} role={isPrimary ? "status" : undefined}>
      <Icon className="top-alert__icon" aria-hidden="true" strokeWidth={2} />
      <span className="top-alert__message">{message}</span>
    </div>
  );
}
