import { useEffect, useState } from "react";
import { CopyRegular, DismissRegular, SquareRegular, SubtractRegular } from "@fluentui/react-icons";
import { Button } from "@/components/ui/button";
import { appWindow, isTauri } from "@/lib/tauri";

export function Titlebar() {
  if (!isTauri) return null;
  return (
    <div data-tauri-drag-region aria-label="Move window" className="pointer-events-auto absolute top-0 left-0 z-40 h-8 w-72" />
  );
}

export function WindowControls() {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (!isTauri) return;
    const win = appWindow();
    let unlisten: (() => void) | undefined;
    let disposed = false;
    const update = () => void win.isMaximized().then((value) => { if (!disposed) setMaximized(value); });
    update();
    void win.onResized(update).then((fn) => { if (disposed) fn(); else unlisten = fn; });
    return () => { disposed = true; unlisten?.(); };
  }, []);

  if (!isTauri) return null;

  return (
    <div className="window-controls flex h-full items-center gap-0" aria-label="Window controls">
      <Button variant="ghost" aria-label="Minimize" className="window-control" onClick={() => void appWindow().minimize()}>
        <SubtractRegular />
      </Button>
      <Button variant="ghost" aria-label={maximized ? "Restore" : "Maximize"} className="window-control" onClick={() => void appWindow().toggleMaximize()}>
        {maximized ? <CopyRegular /> : <SquareRegular />}
      </Button>
      <Button variant="ghost" aria-label="Close" className="window-control window-control-close" onClick={() => void appWindow().close()}>
        <DismissRegular />
      </Button>
    </div>
  );
}