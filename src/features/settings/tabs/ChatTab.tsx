import { useEffect, useState } from "react";
import { HardDriveIcon, Loader2Icon, Trash2Icon } from "lucide-react";
import { toast } from "sonner";
import { Toggle } from "@/features/v3-shell/components/Toggle";
import { clearLocalMediaCache, mediaCacheStats, type MediaCacheStats } from "@/lib/media-cache";
import { formattedBytes } from "@/lib/media";
import { useAlerts } from "@/stores/alerts";
import { useAuth } from "@/stores/auth";
import { usePreferences } from "@/stores/preferences";

export function ChatTab() {
  const userId = useAuth((state) => state.userId);
  const enterToSend = usePreferences((state) => state.enterToSend);
  const compactMessages = usePreferences((state) => state.compactMessages);
  const showMediaPreviews = usePreferences((state) => state.showMediaPreviews);
  const setPreference = usePreferences((state) => state.setPreference);
  const [cacheStats, setCacheStats] = useState<MediaCacheStats | null>(null);
  const [clearing, setClearing] = useState(false);

  const refreshCacheStats = async () => {
    if (!userId) return;
    try {
      setCacheStats(await mediaCacheStats(userId));
    } catch (error) {
      console.warn("Could not read local media cache stats", error);
    }
  };

  useEffect(() => {
    void refreshCacheStats();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [userId]);

  const clearCache = async () => {
    if (!userId) return;
    setClearing(true);
    try {
      await clearLocalMediaCache(userId);
      await refreshCacheStats();
      toast.success("Local media cache cleared.");
    } catch {
      useAlerts.getState().show({ severity: "danger", message: "Couldn't clear the local media cache." });
    } finally {
      setClearing(false);
    }
  };

  return (
    <div className="v3-settings__tab-panel">
      <div className="v3-settings__heading">
        <h2>Chat</h2>
        <p>Message behavior and local attachment storage.</p>
      </div>

      <div className="v3-settings__panel">
        <div className="v3-settings__row">
          <HardDriveIcon aria-hidden="true" className="v3-settings__row-icon" />
          <div className="v3-settings__row-copy">
            <p className="v3-settings__row-title">30-day local media cache</p>
            <p className="v3-settings__row-desc">
              Compressed chat images and videos remain on this device for 30 days, even after the temporary
              server copy is purged.
            </p>
            <p className="v3-settings__row-desc">
              {cacheStats
                ? formattedBytes(cacheStats.bytes) + " in " + cacheStats.entries + " files / " + formattedBytes(cacheStats.limitBytes) + " limit"
                : "Calculating local usage…"}
            </p>
          </div>
          <button
            type="button"
            className="v3-settings__ghost-button"
            disabled={clearing || !cacheStats?.entries}
            onClick={() => void clearCache()}
          >
            {clearing ? <Loader2Icon aria-hidden="true" /> : <Trash2Icon aria-hidden="true" />}
            Clear
          </button>
        </div>
      </div>

      <div className="v3-settings__panel">
        <div className="v3-settings__row">
          <div className="v3-settings__row-copy">
            <p className="v3-settings__row-title">Enter to send</p>
            <p className="v3-settings__row-desc">When off, use Ctrl+Enter to send.</p>
          </div>
          <Toggle checked={enterToSend} onChange={(value) => setPreference("enterToSend", value)} label="Enter to send" />
        </div>
        <div className="v3-settings__row">
          <div className="v3-settings__row-copy">
            <p className="v3-settings__row-title">Compact message spacing</p>
            <p className="v3-settings__row-desc">Fit more conversation history on screen.</p>
          </div>
          <Toggle checked={compactMessages} onChange={(value) => setPreference("compactMessages", value)} label="Compact message spacing" />
        </div>
        <div className="v3-settings__row">
          <div className="v3-settings__row-copy">
            <p className="v3-settings__row-title">Show media previews</p>
            <p className="v3-settings__row-desc">Render images and videos directly in the conversation.</p>
          </div>
          <Toggle checked={showMediaPreviews} onChange={(value) => setPreference("showMediaPreviews", value)} label="Show media previews" />
        </div>
      </div>
    </div>
  );
}
