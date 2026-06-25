import type { AssetRefs, PoolKind, SortDirection } from "./base";

export type DisplayRecord = {
  record_id: string;
  source_order: number;
  record_type: string;
  time?: string | null;
  pool_kind: PoolKind;
  pool_id: string;
  pool_label: string;
  banner: ResolvedBanner;
  item_id: string;
  item_name: string;
  item_asset_refs: AssetRefs;
  item_kind: ItemKind;
  rarity?: number | null;
  count?: number | null;
  roll_points?: number | null;
  roll_label_id?: string | null;
  roll_label?: string | null;
  roll_bucket: RollBucket;
  fork_result_mark?: ForkResultMark | null;
  secondary_item_id?: string | null;
  secondary_item_name?: string | null;
  secondary_item_asset_refs: AssetRefs;
  secondary_count?: number | null;
  derived: RecordDerived;
};

type BannerResolutionIssue =
  | "unknown_pool"
  | "unknown_time"
  | "outside_known_windows"
  | "ambiguous";

type RuleResolutionIssue =
  | "fallback_pool_kind"
  | "missing_banner"
  | "missing_rule"
  | "unsupported_scope";

export type RateUpResult = "up" | "off_rate" | "not_applicable" | "unknown";
export type PityBadge = "fork_up_guarantee" | "fork_5star_guarantee" | "fork_4star_guarantee";
export type ForkResultMark = "win" | "guaranteed" | "lose";
export type RollBucket = "gift" | "sleep" | "1" | "2" | "3" | "4" | "5" | "6" | "not_applicable";
export type ItemKind = "character" | "fork" | "appearance" | "inventory" | "vehicle_module" | "unknown";

export type ResolvedBanner = {
  resolution_issue?: BannerResolutionIssue | null;
  reason?: string | null;
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
  standard_5_pool: string[];
  standard_4_pool: string[];
  rule_id?: string | null;
  asset_refs: AssetRefs;
};

export type GachaRuleView = {
  resolution_issue?: RuleResolutionIssue | null;
  reason?: string | null;
  rule_id?: string | null;
  pool_kind: PoolKind;
  hard_pity_5?: number | null;
  hard_up_pity_5?: number | null;
  pickup_win_rate_5?: number | null;
  has_guarantee_5?: boolean | null;
  guarantee_scope?: string | null;
  carry_scope?: string | null;
};

export type RecordDerived = {
  record_id: string;
  banner_id?: string | null;
  banner_version?: string | null;
  counts_as_pull: boolean;
  global_pull_no?: number | null;
  pull_no_in_pool_kind?: number | null;
  pull_no_in_banner?: number | null;
  pity_5_before: number;
  pity_5_after: number;
  ten_pull_progress_before?: number | null;
  ten_pull_progress_after?: number | null;
  hit_rarity?: number | null;
  rate_up_result: RateUpResult;
  pity_badge?: PityBadge | null;
  guarantee_5_before?: boolean | null;
  guarantee_5_after?: boolean | null;
  fork_up_pity_before?: number | null;
  fork_up_pity_after?: number | null;
  fork_forced_up?: boolean | null;
  rule: GachaRuleView;
};

export type RecordFilter = {
  pool_kind?: PoolKind | null;
  banner_ids?: string[] | null;
  rarities?: number[] | null;
  focused_rarities?: number[] | null;
  rate_up_results?: RateUpResult[] | null;
  roll_buckets?: RollBucket[] | null;
  item_kinds?: ItemKind[] | null;
  fork_result_marks?: ForkResultMark[] | null;
  fork_pity_badges?: PityBadge[] | null;
  date_from?: string | null;
  date_to?: string | null;
  search?: string | null;
  sort_direction?: SortDirection | null;
  limit?: number;
  offset?: number;
};

export type RecordList = {
  total: number;
  records: DisplayRecord[];
};

type RecordBannerOption = {
  banner_id: string;
  pool_kind: PoolKind;
  title: string;
  count: number;
};

type RecordRollBucketOption = {
  bucket: RollBucket;
  count: number;
};

type RecordItemKindOption = {
  item_kind: ItemKind;
  count: number;
};

export type RecordFilterOptions = {
  banners: RecordBannerOption[];
  roll_buckets: RecordRollBucketOption[];
  item_kinds: RecordItemKindOption[];
};
