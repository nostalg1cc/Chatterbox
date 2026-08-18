import { useEffect, useMemo, useState } from "react";
import { PlusIcon, XIcon } from "lucide-react";
import { AVATAR_DECORATIONS, decorationUrl, fetchLiveAvatarDecorations } from "@/lib/avatar-decorations";
import { usePreferences } from "@/stores/preferences";

const INLINE_DECORATION_COUNT = 9;

// Recently-used decorations stay visible inline instead of hiding behind a
// picker dialog - that's the whole point of tracking "recent" in the first
// place. Fills the rest of the strip from the catalog so there's always a
// full row to browse without opening anything. Shared between the General
// settings tab and the onboarding decoration step.
export function AvatarDecorationStrip({
  selected,
  onSelect,
  onBrowseAll,
}: {
  selected: string | null;
  onSelect: (id: string | null) => void;
  onBrowseAll: () => void;
}) {
  const recentDecorationIds = usePreferences((state) => state.recentDecorationIds);
  const setPreference = usePreferences((state) => state.setPreference);
  const [availableDecorations, setAvailableDecorations] = useState(AVATAR_DECORATIONS);

  useEffect(() => {
    fetchLiveAvatarDecorations().then((live) => { if (live.length) setAvailableDecorations(live); }).catch(() => undefined);
  }, []);

  const inlineDecorations = useMemo(() => {
    const seen = new Set<string>();
    const ordered = [...recentDecorationIds, ...availableDecorations.map((item) => item.id)];
    const result: string[] = [];
    for (const id of ordered) {
      if (seen.has(id)) continue;
      seen.add(id);
      result.push(id);
      if (result.length >= INLINE_DECORATION_COUNT) break;
    }
    return result;
  }, [recentDecorationIds, availableDecorations]);

  function selectDecoration(id: string | null) {
    onSelect(id);
    if (!id) return;
    const recent = [id, ...recentDecorationIds.filter((existing) => existing !== id)].slice(0, 5);
    setPreference("recentDecorationIds", recent);
  }

  return (
    <div className="v3-settings__deco-strip">
      <button
        type="button"
        className={"v3-settings__deco-thumb v3-settings__deco-thumb--none" + (selected === null ? " is-active" : "")}
        aria-label="No decoration"
        title="No decoration"
        onClick={() => selectDecoration(null)}
      >
        <XIcon aria-hidden="true" />
      </button>
      {inlineDecorations.map((id) => {
        const item = availableDecorations.find((option) => option.id === id);
        return (
          <button
            key={id}
            type="button"
            className={"v3-settings__deco-thumb" + (selected === id ? " is-active" : "")}
            aria-label={item?.label ?? "Decoration"}
            title={item?.label ?? "Decoration"}
            onClick={() => selectDecoration(id)}
          >
            <img src={decorationUrl(id, false) ?? undefined} alt="" loading="lazy" />
          </button>
        );
      })}
      <button type="button" className="v3-settings__deco-thumb v3-settings__deco-thumb--more" aria-label="Browse all decorations" onClick={onBrowseAll}>
        <PlusIcon aria-hidden="true" />
      </button>
    </div>
  );
}
