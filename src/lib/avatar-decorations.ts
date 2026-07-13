export const AVATAR_DECORATIONS = [
  ["bubble", "Bubble"], ["chun-li", "Chun-Li"], ["dreamy", "Dreamy"], ["heart", "Heart"], ["helly-kitty", "Helly Kitty"], ["leafs", "Leafs"], ["lights", "Lights"], ["milk", "Milk"], ["moon", "Moon"], ["rainbow", "Rainbow"], ["rawr", "RAWR"], ["sparkly", "Sparkly"], ["sparkly-pink", "Sparkly Pink"], ["spider-man", "Spider-Man"], ["toy-story", "Toy Story"], ["venom", "Venom"], ["arcane-anomaly", "Arcane: Anomaly"], ["arcane-boom", "Arcane: Boom"], ["arcane-jynx", "Arcane: Jynx"], ["arcane-powder", "Arcane: Powder"],
] as const;
export type AvatarDecoration = (typeof AVATAR_DECORATIONS)[number][0];
export function decorationUrl(id: string | null | undefined, animated: boolean) { return id ? `/decorations/${id}${animated ? "" : "-still"}.png` : null; }