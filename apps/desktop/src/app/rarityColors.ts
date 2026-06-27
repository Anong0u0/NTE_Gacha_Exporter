const rarityColors = {
  5: "#C59245",
  4: "#915BB1",
  3: "#327EB6",
  unknown: "#c3cec7",
} as const;

export function rarityColor(rarity?: number | null) {
  if (rarity === 5) return rarityColors[5];
  if (rarity === 4) return rarityColors[4];
  if (rarity === 3) return rarityColors[3];
  return rarityColors.unknown;
}

export function rarityClass(rarity?: number | null) {
  if (rarity === 5 || rarity === 4 || rarity === 3) return `rarity-${rarity}`;
  return "rarity-unknown";
}
