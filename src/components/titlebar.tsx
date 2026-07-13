import { useEffect, useState } from "react";
import { CopyIcon, MinusIcon, SquareIcon, XIcon } from "lucide-react";
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
    <div className="ml-1 flex h-full shrink-0">
      <Button variant="ghost" aria-label="Minimize" className="h-full w-11 rounded-none text-muted-foreground hover:text-foreground" onClick={() => void appWindow().minimize()}>
        <MinusIcon className="size-3.5" />
      </Button>
      <Button variant="ghost" aria-label={maximized ? "Restore" : "Maximize"} className="h-full w-11 rounded-none text-muted-foreground hover:text-foreground" onClick={() => void appWindow().toggleMaximize()}>
        {maximized ? <CopyIcon className="size-3" /> : <SquareIcon className="size-3" />}
      </Button>
      <Button variant="ghost" aria-label="Close" className="h-full w-11 rounded-none text-muted-foreground hover:bg-[#c42b1c] hover:text-white" onClick={() => void appWindow().close()}>
        <XIcon className="size-4" />
      </Button>
    </div>
  );
}