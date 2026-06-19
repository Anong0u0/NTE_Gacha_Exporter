import type { AssetRefs, ImportReport, PoolKind, Profile } from "./base";
import type { DisplayRecord, RateUpResult, RuleResolutionStatus } from "./records";

export type PoolKindSummary = {
  pool_kind: PoolKind;
  label: string;
  total_pulls: number;
  roll_points_total: number;
  known_roll_point_records: number;
  missing_roll_point_records: number;
  hit_count: number;
  current_pity: number;
  current_guarantee: boolean;
  hard_pity: number;
  average_5star_pity?: number | null;
  min_5star_pity?: number | null;
  max_5star_pity?: number | null;
  early_hit_count: number;
  up_count: number;
  off_rate_count: number;
  not_applicable_rate_up_count: number;
  unknown_rate_up_count: number;
  observed_up_rate?: number | null;
  latest_5star?: DisplayRecord | null;
  current_4star_pity: number;
  hard_pity_4?: number | null;
  average_4star_pity?: number | null;
  min_4star_pity?: number | null;
  max_4star_pity?: number | null;
  four_star_count: number;
  rate_up_4_count: number;
  off_rate_4_count: number;
  not_applicable_rate_up_4_count: number;
  unknown_rate_up_4_count: number;
  rule_resolution_status: RuleResolutionStatus;
  rule_source_confidence?: string | null;
  average_roll_points_to_5star?: number | null;
  average_roll_points_to_4star?: number | null;
  roll_point_cost_samples_5star: number;
  roll_point_cost_samples_4star: number;
};

export type BannerSummary = {
  banner_id: string;
  pool_id: string;
  pool_kind: PoolKind;
  banner_type?: string | null;
  title: string;
  version?: string | null;
  phase?: string | null;
  start_at?: string | null;
  end_at?: string | null;
  source_confidence?: string | null;
  asset_refs: AssetRefs;
  total_pulls: number;
  roll_points_total: number;
  known_roll_point_records: number;
  missing_roll_point_records: number;
  five_star_count: number;
  four_star_count: number;
  current_5star_pity: number;
  current_4star_pity: number;
  average_5star_pity?: number | null;
  average_4star_pity?: number | null;
  rate_up_5_count: number;
  off_rate_5_count: number;
  not_applicable_rate_up_5_count: number;
  unknown_rate_up_5_count: number;
  rate_up_4_count: number;
  off_rate_4_count: number;
  not_applicable_rate_up_4_count: number;
  unknown_rate_up_4_count: number;
  average_roll_points_to_5star?: number | null;
  average_roll_points_to_4star?: number | null;
  roll_point_cost_samples_5star: number;
  roll_point_cost_samples_4star: number;
  latest_hit?: DisplayRecord | null;
};

export type ResourceSummary = {
  total_roll_points: number;
  known_roll_point_records: number;
  missing_roll_point_records: number;
  by_pool_kind: ResourcePoolKindSummary[];
};

export type ResourcePoolKindSummary = {
  pool_kind: PoolKind;
  label: string;
  roll_points_total: number;
  known_roll_point_records: number;
  missing_roll_point_records: number;
};

export type TimeStats = {
  monthly: TimeBucketSummary[];
  daily: TimeBucketSummary[];
  phases: PhaseSummary[];
  missing_time_records: number;
};

export type TimeBucketSummary = {
  bucket: string;
  total_pulls: number;
  five_star_count: number;
  four_star_count: number;
  roll_points_total: number;
  known_roll_point_records: number;
  missing_roll_point_records: number;
  average_5star_pity?: number | null;
  average_4star_pity?: number | null;
};

export type PhaseSummary = {
  version?: string | null;
  phase?: string | null;
  total_pulls: number;
  five_star_count: number;
  four_star_count: number;
  roll_points_total: number;
  known_roll_point_records: number;
  missing_roll_point_records: number;
  banner_count: number;
  average_5star_pity?: number | null;
  average_4star_pity?: number | null;
};

export type RarityBucket = {
  rarity: number;
  count: number;
  percent: number;
};

export type ItemRank = {
  item_id: string;
  item_name: string;
  rarity?: number | null;
  count: number;
};

export type DashboardOverview = {
  profile: Profile;
  last_run?: ImportReport | null;
  total_records: number;
  pool_kinds: PoolKindSummary[];
  banners: BannerSummary[];
  resource: ResourceSummary;
  time_stats: TimeStats;
  rarity_distribution: RarityBucket[];
  item_ranking: ItemRank[];
  latest_records: DisplayRecord[];
};

export type FiveStarRecord = {
  record: DisplayRecord;
  pity_distance: number;
  result: RateUpResult;
  result_confidence: string;
  guarantee_before?: boolean | null;
  guarantee_after?: boolean | null;
};

export type FourStarRecord = {
  record: DisplayRecord;
  pity_distance: number;
  result: RateUpResult;
  result_confidence: string;
  guarantee_before?: boolean | null;
  guarantee_after?: boolean | null;
};

export type PoolKindDetail = {
  summary: PoolKindSummary;
  five_star_history: FiveStarRecord[];
  four_star_history: FourStarRecord[];
};
