export function HistoryMarker({ children, variant }) {
  if (variant === "system") {
    return (
      <div className="history-marker history-marker--system" role="status">
        <img className="history-marker__cap history-marker__cap--left" src="/history-marker/left.svg" alt="" aria-hidden="true" />
        <div className="history-marker__center">
          <span>{children}</span>
        </div>
        <img className="history-marker__cap history-marker__cap--right" src="/history-marker/right.svg" alt="" aria-hidden="true" />
      </div>
    );
  }
  return <div className="history-marker" role="status"><span>{children}</span></div>;
}
