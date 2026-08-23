import type { AppTheme } from "@/stores/preferences";

// Preview values for the Appearance picker in Settings. The actual live theme
// is applied via document.documentElement.dataset.theme driving the
// [data-theme="..."] CSS overrides in src/features/v3-shell/styles.css. Keep
// the background/material values in sync with that file by hand.
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
  accent: string;
}> = [
  { value: "default", label: "Default", family: "Neutral", bg: "#1e1e1e", material: "#282828", accent: "#6f6f6f" },
  { value: "slate", label: "Slate", family: "Neutral", bg: "hsl(215 18% 17%)", material: "hsl(215 18% 23%)", accent: "hsl(215 52% 62%)" },
  { value: "charcoal", label: "Charcoal", family: "Neutral", bg: "hsl(220 8% 11%)", material: "hsl(220 8% 16%)", accent: "hsl(220 20% 62%)" },
  { value: "graphite", label: "Graphite", family: "Neutral", bg: "hsl(205 13% 15%)", material: "hsl(205 13% 22%)", accent: "hsl(194 46% 58%)" },

  { value: "babyPink", label: "Baby Pink", family: "Pink & Red", bg: "hsl(335 62% 19%)", material: "hsl(335 62% 26%)", accent: "hsl(335 100% 73%)" },
  { value: "blush", label: "Blush", family: "Pink & Red", bg: "hsl(345 70% 19%)", material: "hsl(345 70% 27%)", accent: "hsl(345 100% 71%)" },
  { value: "rose", label: "Rose", family: "Pink & Red", bg: "hsl(320 66% 19%)", material: "hsl(320 66% 27%)", accent: "hsl(320 100% 71%)" },
  { value: "fuchsia", label: "Fuchsia", family: "Pink & Red", bg: "hsl(300 76% 18%)", material: "hsl(300 76% 27%)", accent: "hsl(300 100% 72%)" },
  { value: "crimson", label: "Crimson", family: "Pink & Red", bg: "hsl(355 62% 17%)", material: "hsl(355 62% 25%)", accent: "hsl(355 100% 67%)" },
  { value: "ruby", label: "Ruby", family: "Pink & Red", bg: "hsl(0 70% 17%)", material: "hsl(0 70% 25%)", accent: "hsl(0 100% 68%)" },

  { value: "babyBlue", label: "Baby Blue", family: "Blue & Cyan", bg: "hsl(205 60% 18%)", material: "hsl(205 60% 26%)", accent: "hsl(205 100% 72%)" },
  { value: "sky", label: "Sky", family: "Blue & Cyan", bg: "hsl(200 72% 18%)", material: "hsl(200 72% 26%)", accent: "hsl(200 100% 70%)" },
  { value: "deepBlue", label: "Deep Blue", family: "Blue & Cyan", bg: "hsl(222 66% 15%)", material: "hsl(222 66% 23%)", accent: "hsl(222 100% 69%)" },
  { value: "cobalt", label: "Cobalt", family: "Blue & Cyan", bg: "hsl(226 76% 16%)", material: "hsl(226 76% 25%)", accent: "hsl(226 100% 70%)" },
  { value: "teal", label: "Teal", family: "Blue & Cyan", bg: "hsl(185 56% 15%)", material: "hsl(185 56% 23%)", accent: "hsl(185 100% 65%)" },
  { value: "aqua", label: "Aqua", family: "Blue & Cyan", bg: "hsl(190 72% 16%)", material: "hsl(190 72% 25%)", accent: "hsl(190 100% 70%)" },

  { value: "mint", label: "Mint", family: "Green", bg: "hsl(160 48% 16%)", material: "hsl(160 48% 24%)", accent: "hsl(160 92% 67%)" },
  { value: "emerald", label: "Emerald", family: "Green", bg: "hsl(150 68% 16%)", material: "hsl(150 68% 25%)", accent: "hsl(150 100% 64%)" },
  { value: "forest", label: "Forest", family: "Green", bg: "hsl(140 48% 15%)", material: "hsl(140 48% 23%)", accent: "hsl(140 78% 58%)" },
  { value: "lime", label: "Lime", family: "Green", bg: "hsl(104 50% 16%)", material: "hsl(104 50% 25%)", accent: "hsl(104 100% 67%)" },

  { value: "lavender", label: "Lavender", family: "Purple", bg: "hsl(265 50% 19%)", material: "hsl(265 50% 27%)", accent: "hsl(265 100% 76%)" },
  { value: "violet", label: "Violet", family: "Purple", bg: "hsl(275 68% 18%)", material: "hsl(275 68% 27%)", accent: "hsl(275 100% 73%)" },
  { value: "amethyst", label: "Amethyst", family: "Purple", bg: "hsl(285 65% 18%)", material: "hsl(285 65% 27%)", accent: "hsl(285 100% 74%)" },
  { value: "indigo", label: "Indigo", family: "Purple", bg: "hsl(245 63% 17%)", material: "hsl(245 63% 26%)", accent: "hsl(245 100% 75%)" },

  { value: "sunset", label: "Sunset", family: "Warm", bg: "hsl(28 62% 17%)", material: "hsl(28 62% 25%)", accent: "hsl(28 100% 68%)" },
  { value: "coral", label: "Coral", family: "Warm", bg: "hsl(12 68% 18%)", material: "hsl(12 68% 26%)", accent: "hsl(12 100% 70%)" },
  { value: "gold", label: "Gold", family: "Warm", bg: "hsl(45 62% 17%)", material: "hsl(45 62% 25%)", accent: "hsl(45 100% 66%)" },
  { value: "amber", label: "Amber", family: "Warm", bg: "hsl(38 72% 18%)", material: "hsl(38 72% 26%)", accent: "hsl(38 100% 68%)" },
  { value: "mocha", label: "Mocha", family: "Warm", bg: "hsl(25 30% 16%)", material: "hsl(25 30% 23%)", accent: "hsl(25 72% 63%)" },
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
