import { cn } from "@/lib/utils";

export type TextDecoration = "fuzzy" | "sparkles" | "resize" | "bouncy" | "wavy" | "gradient" | "glitch" | "particle";

export function DecoratedText({ children, effect, active = false, className }: { children: string; effect?: TextDecoration | null; active?: boolean; className?: string }) {
  if (!effect) return <span className={className}>{children}</span>;
  return <span className={cn("text-decoration", `text-decoration-${effect}`, active && "text-decoration-active", className)}>{children}</span>;
}