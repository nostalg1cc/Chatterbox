import { create } from "zustand";
import { CircleAlert, CircleX, TriangleAlert, type LucideIcon } from "lucide-react";

export type AlertSeverity = "neutral" | "warning" | "danger";

export type AlertAction = {
  label: string;
  onClick?: () => void;
  /** Renders as the solid white/black "confirm" button instead of the normal one. */
  confirm?: boolean;
  /** Skip the normal auto-dismiss-on-click - the action itself is
   * responsible for eventually closing or replacing the banner (e.g. an
   * update install that switches the banner into a progress bar instead
   * of just disappearing). */
  keepOpen?: boolean;
};

export type AlertInput = {
  message: string;
  severity?: AlertSeverity;
  icon?: LucideIcon;
  actions?: AlertAction[];
  /** 0-100 to show a progress fill behind the banner, or null/undefined
   * for none. */
  progress?: number | null;
};

export type ActiveAlert = AlertInput & {
  id: string;
  severity: AlertSeverity;
  icon: LucideIcon;
};

const DEFAULT_ICON: Record<AlertSeverity, LucideIcon> = {
  neutral: CircleAlert,
  warning: TriangleAlert,
  danger: CircleX,
};

interface AlertsState {
  active: ActiveAlert | null;
  /** Returns the new alert's id, so a caller that needs to patch() it
   * later (e.g. to report install progress) doesn't have to generate and
   * pass its own. */
  show: (input: AlertInput) => string;
  dismiss: (id: string) => void;
  /** Patches the currently active alert in place (no-op if a different
   * alert has since replaced it) - for live message/progress updates
   * without resetting the banner's enter animation or auto-dismiss timer. */
  patch: (id: string, patch: Partial<AlertInput>) => void;
}

// A single global banner slot, shared app-wide (voice/connection issues,
// keybind conflicts, auth errors, ...) - not scoped to V3Shell, so it works
// on screens like auth that mount before V3Shell ever does. A second show()
// while one is already up simply replaces it; this is deliberately not a
// queue, since stacking multiple full-width banners would be worse than
// just showing whichever is most recent.
export const useAlerts = create<AlertsState>((set) => ({
  active: null,
  show: (input) => {
    const severity = input.severity ?? "neutral";
    const id = `${Date.now()}-${Math.random()}`;
    set({
      active: {
        icon: DEFAULT_ICON[severity],
        ...input,
        id,
        severity,
      },
    });
    return id;
  },
  dismiss: (id) => {
    set((state) => (state.active?.id === id ? { active: null } : state));
  },
  patch: (id, patch) => {
    set((state) => (state.active?.id === id ? { active: { ...state.active, ...patch } } : state));
  },
}));
