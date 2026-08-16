declare module "@/features/v3-shell/components/TopAlert" {
  import type { LucideIcon } from "lucide-react";

  export type TopAlertAction = {
    label: string;
    onClick?: () => void;
    confirm?: boolean;
    keepOpen?: boolean;
  };

  export function TopAlert(props: {
    id: string;
    message: string;
    severity: "neutral" | "warning" | "danger";
    icon: LucideIcon;
    actions?: TopAlertAction[];
    progress?: number | null;
    onDismiss: (id: string) => void;
    onVisibleChange?: (visible: boolean) => void;
  }): import("react").JSX.Element;
}
