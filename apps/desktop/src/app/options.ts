import type { PoolKind } from "../api";

export type ImportMode = "raw" | "public";
export type ExportMode = "json" | "csv";
export type PoolKindFilter = PoolKind | "all";

export const kindOrder: PoolKind[] = ["monopoly_limited", "monopoly_standard", "fork_lottery"];
