import { useEffect, useMemo, useRef, useState } from "react";
import { SearchIcon } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { AVATAR_DECORATIONS, decorationUrl, fetchLiveAvatarDecorations } from "@/lib/avatar-decorations";
import { cn } from "@/lib/utils";
import { usePreferences } from "@/stores/preferences";

export function AvatarDecorationPicker({
  open,
  onOpenChange,
  selected,
  onSelect,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  selected: string | null;
  onSelect: (id: string | null) => void;
}) {
  const recentDecorationIds = usePreferences((state) => state.recentDecorationIds);
  const setPreference = usePreferences((state) => state.setPreference);
  const [search, setSearch] = useState("");
  const [availableDecorations, setAvailableDecorations] = useState(AVATAR_DECORATIONS);
  const gridRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    const refresh = async () => {
      try {
        const live = await fetchLiveAvatarDecorations();
        if (!cancelled && live.length) setAvailableDecorations(live);
      } catch {
        // Cloudinary list delivery can be intentionally restricted; retain the shipped live fallback.
      }
    };
    void refresh();
    const timer = window.setInterval(() => void refresh(), 65_000);
    return () => { cancelled = true; window.clearInterval(timer); };
  }, [open]);

  // Jump straight to whatever's currently selected instead of making the
  // user hunt for it in a grid of however many decorations there are.
  useEffect(() => {
    if (!open) return;
    const frame = requestAnimationFrame(() => {
      if (!selected) return;
      const target = gridRef.current?.querySelector(`[data-decoration-id="${CSS.escape(selected)}"]`);
      target?.scrollIntoView({ block: "center" });
    });
    return () => cancelAnimationFrame(frame);
  }, [open, selected]);

  const filteredDecorations = useMemo(() => {
    const query = search.trim().toLowerCase();
    if (!query) return availableDecorations;
    return availableDecorations.filter((item) => item.label.toLowerCase().includes(query) || item.id.includes(query));
  }, [availableDecorations, search]);

  function selectDecoration(id: string | null) {
    onSelect(id);
    onOpenChange(false);
    if (!id) return;
    const recent = [id, ...recentDecorationIds.filter((existing) => existing !== id)].slice(0, 5);
    setPreference("recentDecorationIds", recent);
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="surface-panel max-w-[720px] border-white/[0.18] bg-[#202020] p-0">
        <DialogHeader className="border-b border-white/[0.10] p-5 pb-4">
          <DialogTitle>Choose an avatar decoration</DialogTitle>
          <DialogDescription>Static previews keep browsing light; your selected decoration animates in the live preview.</DialogDescription>
        </DialogHeader>
        <div className="space-y-4 p-5 pt-4">
          <div className="relative">
            <SearchIcon className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
            <Input value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Search decorations" className="pl-9" autoFocus />
          </div>
          {recentDecorationIds.length > 0 && (
            <div className="space-y-1.5">
              <p className="text-xs text-muted-foreground">Recently used</p>
              <div className="flex gap-2">
                {recentDecorationIds.map((id) => (
                  <button
                    key={id}
                    type="button"
                    title={availableDecorations.find((item) => item.id === id)?.label ?? "Recent decoration"}
                    onClick={() => selectDecoration(id)}
                    className={cn(
                      "group relative size-12 shrink-0 rounded-lg border border-white/[0.12] bg-black/15 transition-colors hover:border-white/[0.32] hover:bg-white/[0.08]",
                      selected === id && "border-white/70 bg-white/[0.12] ring-1 ring-white/[0.18]"
                    )}
                  >
                    <img src={decorationUrl(id, false) ?? undefined} alt="" className="absolute -inset-1 size-[calc(100%+0.5rem)] max-w-none object-contain" loading="lazy" />
                  </button>
                ))}
              </div>
            </div>
          )}
          <div className="flex items-center justify-between text-xs text-muted-foreground">
            <span>{filteredDecorations.length} decorations</span>
            <button type="button" onClick={() => selectDecoration(null)} className="hover:text-foreground">Use none</button>
          </div>
          <div ref={gridRef} className="grid max-h-[52vh] grid-cols-4 gap-x-5 gap-y-6 overflow-y-auto px-2 py-3 sm:grid-cols-6">
            {filteredDecorations.map((item) => (
              <button
                key={item.id}
                type="button"
                data-decoration-id={item.id}
                title={item.label}
                aria-label={item.label}
                onClick={() => selectDecoration(item.id)}
                className={cn(
                  "group relative aspect-square rounded-lg border border-white/[0.12] bg-black/15 transition-colors hover:border-white/[0.32] hover:bg-white/[0.08]",
                  selected === item.id && "border-white/70 bg-white/[0.12] ring-1 ring-white/[0.18]"
                )}
              >
                <img src={decorationUrl(item.id, false) ?? undefined} alt="" className="absolute -inset-1 size-[calc(100%+0.5rem)] max-w-none object-contain" loading="lazy" />
              </button>
            ))}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
