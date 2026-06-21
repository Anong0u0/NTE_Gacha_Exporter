import type { AssetRefs, PoolKind, RecordSortKey, SortDirection } from "./base";

export type DisplayRecord = {
  record_id: string;
  record_type: string;
  time?: string | null;
  pool_kind: PoolKind;
  pool_id: string;
  pool_label: string;
  banner: ResolvedBanner;
  item_id: string;
  item_name: string;
  item_asset_refs: AssetRefs;
  rarity?: number | null;
  count?: number | null;
  roll_points?: number | null;
  secondary_item_id?: string | null;
  secondary_item_name?: string | null;
  secondary_item_asset_refs: AssetRefs;
  secondary_count?: number | null;
  derived: RecordDerived;
};

export type BannerResolutionStatus =
  | "matched"
  | "unknown_pool"
  | "unknown_time"
  | "outside_known_windows"
  | "ambiguous";

export type RuleResolutionStatus =
  | "matched"
  | "fallback_pool_kind"
  | "missing_banner"
  | "missing_rule"
  | "unsupported_scope";

export type RateUpResult = "up" | "off_rate" | "not_applicable" | "unknown";

export type ResolvedBanner = {
  status: BannerResolutionStatus;
  reason: string;
  banner_id?: string | null;
  pool_id?: string | null;
  pool_kind?: PoolKind | string | null;
  banner_type?: "limited" | "standard" | "fork" | string | null;
  title?: string | null;
  version?: string | null;
  start_at?: string | null;
  end_at?: string | null;
  timezone?: string | null;
  rate_up_5: string[];
  rate_up_4: string[];
  rule_id?: string | null;
  asset_refs: AssetRefs;
};

export type GachaRuleView = {
  status: RuleResolutionStatus;
  reason: string;
  rule_id?: string | null;
  pool_kind: PoolKind;
  hard_pity_5?: number | null;
  hard_pity_4?: number | null;
  pickup_win_rate_5?: number | null;
  pickup_win_rate_4?: number | null;
  has_guarantee_5?: boolean | null;
  has_guarantee_4?: boolean | null;
  guarantee_scope?: string | null;
  carry_scope?: string | null;
};

export type RecordDerived = {
  record_id: string;
  banner_id?: string | null;
  banner_version?: string | null;
  pull_no_in_pool_kind: number;
  pull_no_in_banner?: number | null;
  pity_5_before: number;
  pity_5_after: number;
  pity_4_before: number;
  pity_4_after: number;
  hit_rarity?: number | null;
  rate_up_result: RateUpResult;
  guarantee_5_before?: boolean | null;
  guarantee_5_after?: boolean | null;
  guarantee_4_before?: boolean | null;
  guarantee_4_after?: boolean | null;
  rule: GachaRuleView;
};

export type RecordFilter = {
  pool_kind?: PoolKind | null;
  pool_id?: string | null;
  banner_id?: string | null;
  record_type?: string | null;
  rarity?: number | null;
  hit_rarity?: number | null;
  rate_up_result?: RateUpResult | null;
  pity_5_min?: number | null;
  pity_5_max?: number | null;
  pity_4_min?: number | null;
  pity_4_max?: number | null;
  date_from?: string | null;
  date_to?: string | null;
  search?: string | null;
  sort_key?: RecordSortKey | null;
  sort_direction?: SortDirection | null;
  limit?: number;
  offset?: number;
};

export type RecordList = {
  total: number;
  records: DisplayRecord[];
};

export type RecordPoolOption = {
  pool_id: string;
  pool_kind: PoolKind;
  label: string;
  count: number;
};

export type RecordTypeOption = {
  record_type: string;
  count: number;
};

export type RecordBannerOption = {
  banner_id: string;
  pool_kind: PoolKind;
  title: string;
  count: number;
};

export type RecordFilterOptions = {
  pools: RecordPoolOption[];
  banners: RecordBannerOption[];
  record_types: RecordTypeOption[];
};
