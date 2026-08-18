declare module "@/features/v3-shell/components/ActionButton" {
  import type { LucideIcon } from "lucide-react";
  import type { ButtonHTMLAttributes, ReactNode } from "react";

  export function ActionButton(
    props: {
      icon?: LucideIcon;
      image?: string;
      label?: string;
      text?: string;
      className?: string;
      children?: ReactNode;
    } & ButtonHTMLAttributes<HTMLButtonElement>
  ): import("react").JSX.Element;
}
