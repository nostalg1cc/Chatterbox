import { useEffect, useState } from "react";
import { CopyIcon, MinusIcon, SquareIcon, XIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { appWindow, isTauri } from "@/lib/tauri";

export function Titlebar() {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (!isTauri) return;
    const win = appWindow();
    let unlisten: (() => void) | undefined;
    let disposed = false;

    const update = () => {
      void win.isMaximized().then((value) => {
        if (!disposed) setMaximized(value);
      });
    };
    update();
    void win.onResized(update).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  if (!isTauri) return null;

  return (
    <header
      data-tauri-drag-region
      className="flex h-8 shrink-0 items-center justify-between select-none"
    >
      <span
        data-tauri-drag-region
        className="pointer-events-none px-3 text-[11px] font-semibold tracking-[0.2em] text-muted-foreground uppercase"
      >
        Dislight
      </span>
      <div className="flex h-full">
        <Button
          variant="ghost"
          aria-label="Minimize"
          className="h-full w-11 rounded-none text-muted-foreground hover:text-foreground"
          onClick={() => void appWindow().minimize()}
        >
          <MinusIcon className="size-3.5" />
        </Button>
        <Button
          variant="ghost"
          aria-label={maximized ? "Restore" : "Maximize"}
          className="h-full w-11 rounded-none text-muted-foreground hover:text-foreground"
          onClick={() => void appWindow().toggleMaximize()}
        >
          {maximized ? <CopyIcon className="size-3" /> : <SquareIcon className="size-3" />}
        </Button>
        <Button
          variant="ghost"
          aria-label="Close"
          className="h-full w-11 rounded-none text-muted-foreground hover:bg-[#c42b1c] hover:text-white"
          onClick={() => void appWindow().close()}
        >
          <XIcon className="size-4" />
        </Button>
      </div>
    </header>
  );
}
