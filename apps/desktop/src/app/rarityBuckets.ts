import type { DashboardSelectionDetail, PullRarityBucketKey } from "../api";
import type { I18nKey } from "./i18n";

export type RaritySlice = {
  key: string;
  rarity?: number | null;
  className: string;
  label: string;
  count: number;
  percent: number;
  percentText: string;
};

type Translator = (key: I18nKey) => string;

const bucketLabels: Record<PullRarityBucketKey, I18nKey> = {
  five_up: "rarityBucket.fiveUp",
  five_non_up: "rarityBucket.fiveNonUp",
  five_character: "rarityBucket.fiveCharacter",
  five_item: "rarityBucket.fiveItem",
  four_character: "rarityBucket.fourCharacter",
  four_hit: "rarityBucket.fourHit",
  four_item: "rarityBucket.fourItem",
  three: "rarityBucket.three",
  unknown: "rarityBucket.unknown",
};

const bucketClasses: Record<PullRarityBucketKey, string> = {
  five_up: "rarity-5-up",
  five_non_up: "rarity-5-non-up",
  five_character: "rarity-5-character",
  five_item: "rarity-5-item",
  four_character: "rarity-4-character",
  four_hit: "rarity-4-hit",
  four_item: "rarity-4-item",
  three: "rarity-3",
  unknown: "rarity-unknown",
};

export function dashboardRaritySlices(detail?: DashboardSelectionDetail | null, t?: Translator): RaritySlice[] {
  if (!detail) return [];
  return (detail.pull_rarity_distribution ?? []).map((bucket) => {
    const percent = bucket.percent ?? 0;
    return {
      key: bucket.key,
      rarity: bucket.rarity,
      className: bucketClasses[bucket.key] ?? "rarity-unknown",
      label: t ? t(bucketLabels[bucket.key]) : fallbackBucketLabel(bucket.key),
      count: bucket.count,
      percent,
      percentText: formatCompactPercent(percent),
    };
  });
}

function fallbackBucketLabel(key: PullRarityBucketKey) {
  switch (key) {
    case "five_up":
      return "5★UP";
    case "five_non_up":
      return "5★ Non-UP";
    case "five_character":
      return "5★ Character";
    case "five_item":
      return "5★ Item";
    case "four_character":
      return "4★ Character";
    case "four_hit":
      return "4★";
    case "four_item":
      return "4★ Item";
    case "three":
      return "3★";
    case "unknown":
      return "Unknown";
  }
}

function formatCompactPercent(value?: number | null) {
  if (value === null || value === undefined) return "-";
  return `${Number((value * 100).toFixed(2))}%`;
}
