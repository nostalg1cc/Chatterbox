import { Loader2Icon } from "lucide-react";

export function BootScreen() {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-4">
      <span className="text-lg font-semibold tracking-[0.35em] text-foreground uppercase">
        Dislight
      </span>
      <Loader2Icon className="size-4 animate-spin text-muted-foreground" />
    </div>
  );
}
