import type { AssetRefs, ImportReport, PoolKind, Profile } from "./base";
import type { DisplayRecord, RateUpResult, RecordFilterOptions, RecordList } from "./records";

export type PoolKindSummary = {
  pool_kind: PoolKind;
  label: string;
  total_pulls: number;
  roll_points_total: number;
  known_roll_point_records: number;
  missing_roll_point_records: number;
  hit_count: number;
  five_star_item_count: number;
  current_pity: number;
  current_ten_pull_progress?: number | null;
  current_guarantee: boolean;
  hard_pity: number;
  average_5star_pity?: number | null;
  average_4star_pity?: number | null;
  min_5star_pity?: number | null;
  max_5star_pity?: number | null;
  early_hit_count: number;
  up_count: number;
  off_rate_count: number;
  not_applicable_rate_up_count: number;
  unknown_rate_up_count: number;
  observed_up_rate?: number | null;
  fork_win_count: number;
  fork_loss_count: number;
  fork_forced_up_count: number;
  fork_observed_25_75_win_rate?: number | null;
  latest_5star?: DisplayRecord | null;
  four_star_count: number;
  rate_up_4_count: number;
  off_rate_4_count: number;
  not_applicable_rate_up_4_count: number;
  unknown_rate_up_4_count: number;
  average_roll_points_to_5star?: number | null;
  roll_point_cost_samples_5star: number;
};

export type BannerSummary = {
  banner_id: string;
  pool_id: string;
  pool_kind: PoolKind;
  banner_type?: string | null;
  title: string;
  version?: string | null;
  start_at?: string | null;
  end_at?: string | null;
  asset_refs: AssetRefs;
  total_pulls: number;
  roll_points_total: number;
  known_roll_point_records: number;
  missing_roll_point_records: number;
  five_star_count: number;
  four_star_count: number;
  current_5star_pity: number;
  average_5star_pity?: number | null;
  rate_up_5_count: number;
  off_rate_5_count: number;
  not_applicable_rate_up_5_count: number;
  unknown_rate_up_5_count: number;
  fork_win_count: number;
  fork_loss_count: number;
  fork_forced_up_count: number;
  fork_observed_25_75_win_rate?: number | null;
  rate_up_4_count: number;
  off_rate_4_count: number;
  not_applicable_rate_up_4_count: number;
  unknown_rate_up_4_count: number;
  average_roll_points_to_5star?: number | null;
  roll_point_cost_samples_5star: number;
  latest_hit?: DisplayRecord | null;
};

export type TimeStats = {
  monthly: TimeBucketSummary[];
  daily: TimeBucketSummary[];
  missing_time_records: number;
};

type TimeBucketSummary = {
  bucket: string;
  total_pulls: number;
  five_star_count: number;
  four_star_count: number;
  roll_points_total: number;
  known_roll_point_records: number;
  missing_roll_point_records: number;
  average_5star_pity?: number | null;
};

type RarityBucket = {
  rarity: number;
  count: number;
  percent: number;
};

export type PullRarityBucketKey =
  | "five_up"
  | "five_non_up"
  | "five_character"
  | "five_item"
  | "four_character"
  | "four_hit"
  | "four_item"
  | "three"
  | "unknown";

export type PullRarityBucket = {
  key: PullRarityBucketKey;
  rarity?: number | null;
  count: number;
  percent: number;
};

export type ItemRank = {
  item_id: string;
  item_name: string;
  item_asset_refs: AssetRefs;
  rarity?: number | null;
  count: number;
};

export type DashboardOverview = {
  profile: Profile;
  last_run?: ImportReport | null;
  total_records: number;
  pool_kinds: PoolKindSummary[];
  banners: BannerSummary[];
  time_stats: TimeStats;
  rarity_distribution: RarityBucket[];
  item_ranking: ItemRank[];
};

type FiveStarRecord = {
  record: DisplayRecord;
  pity_distance: number;
  result: RateUpResult;
  guarantee_before?: boolean | null;
  guarantee_after?: boolean | null;
};

export type PoolKindDetail = {
  summary: PoolKindSummary;
  five_star_history: FiveStarRecord[];
};

export type DashboardSelection =
  | { kind: "pool_kind"; pool_kind: PoolKind }
  | { kind: "banner"; pool_kind: PoolKind; banner_id: string };

export type DashboardSelectionDetail = PoolKindDetail & {
  rarity_distribution: RarityBucket[];
  hit_rarity_distribution: RarityBucket[];
  pull_rarity_distribution: PullRarityBucket[];
  item_ranking: ItemRank[];
};

export type ProfileAnalysisView = {
  overview: DashboardOverview;
  selected_detail: DashboardSelectionDetail;
  record_filter_options: RecordFilterOptions;
  record_page: RecordList;
};
