import type { AppTheme } from "@/stores/preferences";

// Preview-only values for the Appearance picker in Settings - the actual
// live theme is applied via document.documentElement.dataset.theme driving
// the [data-theme="..."] CSS variable overrides in
// src/features/v3-shell/styles.css. Keep these in sync with that file by
// hand; there's no single source of truth shared between CSS and JS here.
export const THEME_OPTIONS: Array<{
  value: AppTheme;
  label: string;
  bg: string;
  material: string;
}> = [
  { value: "default", label: "Default", bg: "#1e1e1e", material: "#282828" },
  { value: "babyPink", label: "Baby Pink", bg: "hsl(335 34% 15%)", material: "hsl(335 34% 20%)" },
  { value: "babyBlue", label: "Baby Blue", bg: "hsl(205 34% 14%)", material: "hsl(205 34% 19%)" },
  { value: "deepBlue", label: "Deep Blue", bg: "hsl(222 45% 11%)", material: "hsl(222 45% 16%)" },
  { value: "lavender", label: "Lavender", bg: "hsl(265 30% 15%)", material: "hsl(265 30% 20%)" },
  { value: "mint", label: "Mint", bg: "hsl(160 26% 12%)", material: "hsl(160 26% 17%)" },
];
