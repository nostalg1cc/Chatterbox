import { supabase } from "@/lib/supabase";
import { useAlerts, type AlertSeverity } from "@/stores/alerts";

interface AppNotice {
  id: string;
  severity: string;
  message: string;
}

const DISMISSED_KEY = "dislight-dismissed-notice-id";
const VALID_SEVERITIES = new Set<AlertSeverity>(["neutral", "warning", "danger"]);

function showIfNew(notice: AppNotice) {
  if (window.localStorage.getItem(DISMISSED_KEY) === notice.id) return;
  const severity = VALID_SEVERITIES.has(notice.severity as AlertSeverity)
    ? (notice.severity as AlertSeverity)
    : "neutral";
  useAlerts.getState().show({
    severity,
    message: notice.message,
    actions: [
      { label: "Dismiss", onClick: () => window.localStorage.setItem(DISMISSED_KEY, notice.id) },
    ],
  });
}

/**
 * A lightweight way to push a message into every signed-in, live app
 * instance without shipping a release - see supabase/functions/push-notice.
 * Shows the most recent notice on load (unless already dismissed on this
 * device), then live ones as they're pushed while the app stays open.
 */
export function subscribeToAppNotices(): () => void {
  let cancelled = false;
  void supabase
    .from("app_notices")
    .select("id, severity, message")
    .order("created_at", { ascending: false })
    .limit(1)
    .maybeSingle()
    .then(({ data }) => {
      if (!cancelled && data) showIfNew(data as AppNotice);
    });

  const channel = supabase
    .channel("app-notices")
    .on(
      "postgres_changes",
      { event: "INSERT", schema: "public", table: "app_notices" },
      (payload) => showIfNew(payload.new as AppNotice)
    )
    .subscribe();

  return () => {
    cancelled = true;
    void supabase.removeChannel(channel);
  };
}
