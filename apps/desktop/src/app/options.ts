import type { PoolKind, RateUpResult } from "../api";

export type ImportMode = "raw" | "public";
export type ExportMode = "json" | "csv";
export type PoolKindFilter = PoolKind | "all";
export type HitRarityFilter = "" | "4" | "5";
export type RateUpFilter = "" | RateUpResult;

export const kindOrder: PoolKind[] = ["monopoly_limited", "monopoly_standard", "fork_lottery"];
export const kindLabels: Record<PoolKind, string> = {
  monopoly_limited: "Limited",
  monopoly_standard: "Standard",
  fork_lottery: "Fork",
};
