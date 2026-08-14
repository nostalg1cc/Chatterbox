import { create } from "zustand";
import { CircleAlert, CircleX, TriangleAlert, type LucideIcon } from "lucide-react";

export type AlertSeverity = "neutral" | "warning" | "danger";

export type AlertAction = {
  label: string;
  onClick?: () => void;
  /** Renders as the solid white/black "confirm" button instead of the normal one. */
  confirm?: boolean;
};

export type AlertInput = {
  message: string;
  severity?: AlertSeverity;
  icon?: LucideIcon;
  actions?: AlertAction[];
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
  show: (input: AlertInput) => void;
  dismiss: (id: string) => void;
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
    set({
      active: {
        id: `${Date.now()}-${Math.random()}`,
        icon: DEFAULT_ICON[severity],
        ...input,
        severity,
      },
    });
  },
  dismiss: (id) => {
    set((state) => (state.active?.id === id ? { active: null } : state));
  },
}));
