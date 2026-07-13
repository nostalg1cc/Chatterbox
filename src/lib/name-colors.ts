import type { NameColor } from "@/lib/types";

export const NAME_COLOR_OPTIONS: Array<{
  value: NameColor;
  label: string;
  swatch: string;
}> = [
  { value: "default", label: "Default", swatch: "bg-zinc-200" },
  { value: "slate", label: "Slate", swatch: "bg-slate-300" },
  { value: "red", label: "Red", swatch: "bg-red-400" },
  { value: "orange", label: "Orange", swatch: "bg-orange-400" },
  { value: "amber", label: "Amber", swatch: "bg-amber-400" },
  { value: "green", label: "Green", swatch: "bg-emerald-400" },
  { value: "cyan", label: "Cyan", swatch: "bg-cyan-400" },
  { value: "blue", label: "Blue", swatch: "bg-blue-400" },
  { value: "violet", label: "Violet", swatch: "bg-violet-400" },
  { value: "pink", label: "Pink", swatch: "bg-pink-400" },
];

const NAME_COLOR_CLASSES: Record<NameColor, string> = {
  default: "text-foreground",
  slate: "text-slate-300",
  red: "text-red-400",
  orange: "text-orange-400",
  amber: "text-amber-400",
  green: "text-emerald-400",
  cyan: "text-cyan-400",
  blue: "text-blue-400",
  violet: "text-violet-400",
  pink: "text-pink-400",
};

export function nameColorClass(color: NameColor | null | undefined) {
  return NAME_COLOR_CLASSES[color ?? "default"];
}
