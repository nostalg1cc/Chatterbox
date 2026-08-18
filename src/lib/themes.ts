import type { AppTheme } from "@/stores/preferences";

// Preview-only values for the Appearance picker in Settings - the actual
// live theme is applied via document.documentElement.dataset.theme driving
// the [data-theme="..."] CSS variable overrides in
// src/features/v3-shell/styles.css. Keep these in sync with that file by
// hand; there's no single source of truth shared between CSS and JS here.
//
// `family` groups themes into a color spectrum (e.g. Pink & Red running
// pink -> magenta -> deep red) for the picker, rather than one flat grid of
// unrelated hues - order within a family should read as a shade progression.
export const THEME_OPTIONS: Array<{
  value: AppTheme;
  label: string;
  family: string;
  bg: string;
  material: string;
}> = [
  { value: "default", label: "Default", family: "Neutral", bg: "#1e1e1e", material: "#282828" },
  { value: "slate", label: "Slate", family: "Neutral", bg: "hsl(215 14% 14%)", material: "hsl(215 14% 19%)" },
  { value: "charcoal", label: "Charcoal", family: "Neutral", bg: "hsl(220 6% 9%)", material: "hsl(220 6% 14%)" },

  { value: "babyPink", label: "Baby Pink", family: "Pink & Red", bg: "hsl(335 34% 15%)", material: "hsl(335 34% 20%)" },
  { value: "rose", label: "Rose", family: "Pink & Red", bg: "hsl(320 40% 15%)", material: "hsl(320 40% 20%)" },
  { value: "crimson", label: "Crimson", family: "Pink & Red", bg: "hsl(355 38% 13%)", material: "hsl(355 38% 18%)" },

  { value: "babyBlue", label: "Baby Blue", family: "Blue", bg: "hsl(205 34% 14%)", material: "hsl(205 34% 19%)" },
  { value: "deepBlue", label: "Deep Blue", family: "Blue", bg: "hsl(222 45% 11%)", material: "hsl(222 45% 16%)" },
  { value: "teal", label: "Teal", family: "Blue", bg: "hsl(185 35% 12%)", material: "hsl(185 35% 17%)" },

  { value: "mint", label: "Mint", family: "Green", bg: "hsl(160 26% 12%)", material: "hsl(160 26% 17%)" },
  { value: "forest", label: "Forest", family: "Green", bg: "hsl(140 28% 12%)", material: "hsl(140 28% 17%)" },

  { value: "lavender", label: "Lavender", family: "Purple", bg: "hsl(265 30% 15%)", material: "hsl(265 30% 20%)" },
  { value: "violet", label: "Violet", family: "Purple", bg: "hsl(275 55% 16%)", material: "hsl(275 55% 22%)" },

  { value: "sunset", label: "Sunset", family: "Warm", bg: "hsl(28 40% 13%)", material: "hsl(28 40% 18%)" },
  { value: "gold", label: "Gold", family: "Warm", bg: "hsl(45 45% 13%)", material: "hsl(45 45% 18%)" },
  { value: "mocha", label: "Mocha", family: "Warm", bg: "hsl(25 20% 14%)", material: "hsl(25 20% 19%)" },
];

export function themesByFamily() {
  const families = new Map<string, typeof THEME_OPTIONS>();
  for (const option of THEME_OPTIONS) {
    const list = families.get(option.family) ?? [];
    list.push(option);
    families.set(option.family, list);
  }
  return [...families.entries()];
}
