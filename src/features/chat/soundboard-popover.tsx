import { useEffect, useState } from "react";
import { Loader2Icon, Music2Icon } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Popover,
  PopoverContent,
  PopoverDescription,
  PopoverHeader,
  PopoverTitle,
  PopoverTrigger,
} from "@/components/ui/popover";
import { useAuth } from "@/stores/auth";
import { useSoundboard, type SoundboardSound } from "@/stores/soundboard";

interface SoundboardPopoverProps {
  conversationId: string;
  partnerName: string;
}

export function SoundboardPopover({
  conversationId,
  partnerName,
}: SoundboardPopoverProps) {
  const userId = useAuth((state) => state.userId);
  const sounds = useSoundboard((state) => state.availableSounds);
  const loading = useSoundboard((state) => state.loading);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    const show = () => setOpen(true);
    window.addEventListener("dislight:open-soundboard", show);
    return () => window.removeEventListener("dislight:open-soundboard", show);
  }, []);

  useEffect(() => {
    if (open) void useSoundboard.getState().loadAvailable(conversationId);
  }, [conversationId, open]);

  const ownSounds = sounds.filter((sound) => sound.owner_id === userId);
  const partnerSounds = sounds.filter((sound) => sound.owner_id !== userId);

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label="Open shared soundboard"
          className="text-muted-foreground"
        >
          <Music2Icon />
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-96 border border-white/[0.16] p-3">
        <PopoverHeader>
          <PopoverTitle>Shared soundboard</PopoverTitle>
          <PopoverDescription>
            Your library and {partnerName}&apos;s—playable by either of you.
          </PopoverDescription>
        </PopoverHeader>
        {loading ? (
          <div className="flex h-28 items-center justify-center">
            <Loader2Icon className="size-4 animate-spin text-muted-foreground" />
          </div>
        ) : sounds.length ? (
          <div className="max-h-80 space-y-4 overflow-y-auto pr-1">
            <SoundGroup title="Yours" sounds={ownSounds} />
            <SoundGroup title={partnerName + "'s"} sounds={partnerSounds} />
          </div>
        ) : (
          <p className="rounded-md border border-dashed border-white/[0.13] p-5 text-center text-xs text-muted-foreground">
            Neither of you has added a sound yet. Add yours in User Settings.
          </p>
        )}
      </PopoverContent>
    </Popover>
  );
}

function SoundGroup({
  title,
  sounds,
}: {
  title: string;
  sounds: SoundboardSound[];
}) {
  if (!sounds.length) return null;
  return (
    <section>
      <h3 className="mb-1.5 px-0.5 text-[10px] font-semibold tracking-[0.12em] text-muted-foreground uppercase">
        {title}
      </h3>
      <div className="grid grid-cols-2 gap-2">
        {sounds.map((sound) => (
          <button
            key={sound.id}
            type="button"
            className="min-w-0 rounded-md border border-white/[0.13] bg-muted/35 px-3 py-2.5 text-left transition-colors hover:border-white/[0.22] hover:bg-muted/70 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            onClick={() => void useSoundboard.getState().play(sound.id)}
          >
            <span className="block truncate text-xs font-medium">{sound.name}</span>
            <span className="mt-0.5 block text-[10px] text-muted-foreground">
              {(sound.duration_ms / 1000).toFixed(1)}s
            </span>
          </button>
        ))}
      </div>
    </section>
  );
}