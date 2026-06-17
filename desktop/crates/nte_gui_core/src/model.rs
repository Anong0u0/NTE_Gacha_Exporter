use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GuiError {
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid document: {0}")]
    InvalidDocument(String),
    #[error("invalid profile: {0}")]
    InvalidProfile(String),
    #[error("profile not found: {0}")]
    ProfileNotFound(String),
    #[error("unknown pool_id: {0}")]
    UnknownPoolId(String),
    #[error("locale not found: {0}")]
    LocaleNotFound(String),
    #[error("invalid backup: {0}")]
    InvalidBackup(String),
    #[error("invalid update: {0}")]
    InvalidUpdate(String),
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Settings {
    pub active_profile: String,
    pub locale: String,
    pub update_channel: String,
    pub check_updates_on_startup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SettingsPatch {
    pub active_profile: Option<String>,
    pub locale: Option<String>,
    pub update_channel: Option<String>,
    pub check_updates_on_startup: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Profile {
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportReport {
    pub profile_name: String,
    pub source_kind: String,
    pub source_path: Option<String>,
    pub records_seen: u64,
    pub records_inserted: u64,
    pub records_skipped: u64,
    pub completed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackupReport {
    pub path: String,
    pub profile_count: u64,
    pub record_count: u64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RestoreReport {
    pub source_path: String,
    pub profiles_seen: u64,
    pub profiles_created: u64,
    pub profiles_merged: u64,
    pub records_seen: u64,
    pub records_inserted: u64,
    pub records_skipped: u64,
    pub settings_restored: bool,
    pub completed_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpdateChannel {
    Stable,
    Beta,
}

impl UpdateChannel {
    pub fn from_settings(value: &str) -> Self {
        if value.eq_ignore_ascii_case("beta") {
            Self::Beta
        } else {
            Self::Stable
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Beta => "beta",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateManifest {
    pub schema: String,
    pub schema_version: u32,
    pub version: String,
    pub channel: UpdateChannel,
    pub release_url: String,
    pub asset_name: String,
    pub download_url: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdatePackage {
    pub version: String,
    pub channel: UpdateChannel,
    pub release_url: String,
    pub asset_name: String,
    pub download_url: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateStatus {
    pub portable_root: String,
    pub current_version: String,
    pub supported_layout: bool,
    pub staged_version: Option<String>,
    pub rollback_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateCheckReport {
    pub current_version: String,
    pub channel: UpdateChannel,
    pub available: bool,
    pub package: Option<UpdatePackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateStageReport {
    pub package: UpdatePackage,
    pub archive_path: String,
    pub staging_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateInstallPlan {
    pub root: String,
    pub version: String,
    pub staging_path: String,
    pub helper_path: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum PoolKind {
    MonopolyLimited,
    MonopolyStandard,
    ForkLottery,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BannerResolutionStatus {
    Matched,
    UnknownPool,
    UnknownTime,
    OutsideKnownWindows,
    Ambiguous,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuleResolutionStatus {
    Matched,
    FallbackPoolKind,
    MissingBanner,
    MissingRule,
    UnsupportedScope,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RateUpResult {
    Up,
    OffRate,
    NotApplicable,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolvedBanner {
    pub status: BannerResolutionStatus,
    pub reason: String,
    pub banner_id: Option<String>,
    pub pool_id: Option<String>,
    pub pool_kind: Option<String>,
    pub banner_type: Option<String>,
    pub title: Option<String>,
    pub version: Option<String>,
    pub phase: Option<String>,
    pub start_at: Option<String>,
    pub end_at: Option<String>,
    pub timezone: Option<String>,
    pub rate_up_5: Vec<String>,
    pub rate_up_4: Vec<String>,
    pub rule_id: Option<String>,
    pub asset_refs: BTreeMap<String, serde_json::Value>,
    pub source_confidence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GachaRuleView {
    pub status: RuleResolutionStatus,
    pub reason: String,
    pub rule_id: Option<String>,
    pub pool_kind: PoolKind,
    pub hard_pity_5: Option<u64>,
    pub hard_pity_4: Option<u64>,
    pub pickup_win_rate_5: Option<u8>,
    pub pickup_win_rate_4: Option<u8>,
    pub has_guarantee_5: Option<bool>,
    pub has_guarantee_4: Option<bool>,
    pub guarantee_scope: Option<String>,
    pub carry_scope: Option<String>,
    pub source_confidence: Option<String>,
}

impl PoolKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MonopolyLimited => "monopoly_limited",
            Self::MonopolyStandard => "monopoly_standard",
            Self::ForkLottery => "fork_lottery",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InternalRecord {
    pub record_id: String,
    pub record_type: String,
    pub time: Option<String>,
    pub pool_id: String,
    pub item_id: String,
    pub count: Option<i64>,
    pub roll_points: Option<i64>,
    pub secondary_item_id: Option<String>,
    pub secondary_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DashboardOverview {
    pub profile: Profile,
    pub last_run: Option<ImportReport>,
    pub total_records: u64,
    pub pool_kinds: Vec<PoolKindSummary>,
    pub banners: Vec<BannerSummary>,
    pub resource: ResourceSummary,
    pub time_stats: TimeStats,
    pub rarity_distribution: Vec<RarityBucket>,
    pub item_ranking: Vec<ItemRank>,
    pub latest_records: Vec<DisplayRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BannerSummary {
    pub banner_id: String,
    pub pool_id: String,
    pub pool_kind: PoolKind,
    pub banner_type: Option<String>,
    pub title: String,
    pub version: Option<String>,
    pub phase: Option<String>,
    pub start_at: Option<String>,
    pub end_at: Option<String>,
    pub source_confidence: Option<String>,
    pub asset_refs: BTreeMap<String, serde_json::Value>,
    pub total_pulls: u64,
    pub roll_points_total: i64,
    pub known_roll_point_records: u64,
    pub missing_roll_point_records: u64,
    pub five_star_count: u64,
    pub four_star_count: u64,
    pub current_5star_pity: u64,
    pub current_4star_pity: u64,
    pub average_5star_pity: Option<f64>,
    pub average_4star_pity: Option<f64>,
    pub rate_up_5_count: u64,
    pub off_rate_5_count: u64,
    pub not_applicable_rate_up_5_count: u64,
    pub unknown_rate_up_5_count: u64,
    pub rate_up_4_count: u64,
    pub off_rate_4_count: u64,
    pub not_applicable_rate_up_4_count: u64,
    pub unknown_rate_up_4_count: u64,
    pub average_roll_points_to_5star: Option<f64>,
    pub average_roll_points_to_4star: Option<f64>,
    pub roll_point_cost_samples_5star: u64,
    pub roll_point_cost_samples_4star: u64,
    pub latest_hit: Option<DisplayRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceSummary {
    pub total_roll_points: i64,
    pub known_roll_point_records: u64,
    pub missing_roll_point_records: u64,
    pub by_pool_kind: Vec<ResourcePoolKindSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourcePoolKindSummary {
    pub pool_kind: PoolKind,
    pub label: String,
    pub roll_points_total: i64,
    pub known_roll_point_records: u64,
    pub missing_roll_point_records: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimeStats {
    pub monthly: Vec<TimeBucketSummary>,
    pub daily: Vec<TimeBucketSummary>,
    pub phases: Vec<PhaseSummary>,
    pub missing_time_records: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimeBucketSummary {
    pub bucket: String,
    pub total_pulls: u64,
    pub five_star_count: u64,
    pub four_star_count: u64,
    pub roll_points_total: i64,
    pub known_roll_point_records: u64,
    pub missing_roll_point_records: u64,
    pub average_5star_pity: Option<f64>,
    pub average_4star_pity: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PhaseSummary {
    pub version: Option<String>,
    pub phase: Option<String>,
    pub total_pulls: u64,
    pub five_star_count: u64,
    pub four_star_count: u64,
    pub roll_points_total: i64,
    pub known_roll_point_records: u64,
    pub missing_roll_point_records: u64,
    pub banner_count: u64,
    pub average_5star_pity: Option<f64>,
    pub average_4star_pity: Option<f64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RecordSortKey {
    #[default]
    Time,
    Pool,
    Item,
    Rarity,
    RecordType,
    Banner,
    PullNo,
    #[serde(rename = "pity_5")]
    Pity5,
    #[serde(rename = "pity_4")]
    Pity4,
    RateUp,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    Asc,
    #[default]
    Desc,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PoolKindSummary {
    pub pool_kind: PoolKind,
    pub label: String,
    pub total_pulls: u64,
    pub roll_points_total: i64,
    pub known_roll_point_records: u64,
    pub missing_roll_point_records: u64,
    pub hit_count: u64,
    pub current_pity: u64,
    pub current_guarantee: bool,
    pub hard_pity: u64,
    pub average_5star_pity: Option<f64>,
    pub min_5star_pity: Option<u64>,
    pub max_5star_pity: Option<u64>,
    pub early_hit_count: u64,
    pub up_count: u64,
    pub off_rate_count: u64,
    pub not_applicable_rate_up_count: u64,
    pub unknown_rate_up_count: u64,
    pub observed_up_rate: Option<f64>,
    pub latest_5star: Option<DisplayRecord>,
    pub current_4star_pity: u64,
    pub hard_pity_4: Option<u64>,
    pub average_4star_pity: Option<f64>,
    pub min_4star_pity: Option<u64>,
    pub max_4star_pity: Option<u64>,
    pub four_star_count: u64,
    pub rate_up_4_count: u64,
    pub off_rate_4_count: u64,
    pub not_applicable_rate_up_4_count: u64,
    pub unknown_rate_up_4_count: u64,
    pub rule_resolution_status: RuleResolutionStatus,
    pub rule_source_confidence: Option<String>,
    pub average_roll_points_to_5star: Option<f64>,
    pub average_roll_points_to_4star: Option<f64>,
    pub roll_point_cost_samples_5star: u64,
    pub roll_point_cost_samples_4star: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PoolKindDetail {
    pub summary: PoolKindSummary,
    pub five_star_history: Vec<FiveStarRecord>,
    pub four_star_history: Vec<FourStarRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FiveStarRecord {
    pub record: DisplayRecord,
    pub pity_distance: u64,
    pub result: FiveStarResult,
    pub result_confidence: String,
    pub guarantee_before: Option<bool>,
    pub guarantee_after: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FourStarRecord {
    pub record: DisplayRecord,
    pub pity_distance: u64,
    pub result: RateUpResult,
    pub result_confidence: String,
    pub guarantee_before: Option<bool>,
    pub guarantee_after: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FiveStarResult {
    Up,
    OffRate,
    NotApplicable,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RarityBucket {
    pub rarity: u8,
    pub count: u64,
    pub percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ItemRank {
    pub item_id: String,
    pub item_name: String,
    pub rarity: Option<u8>,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordDerived {
    pub record_id: String,
    pub banner_id: Option<String>,
    pub banner_version: Option<String>,
    pub banner_phase: Option<String>,
    pub pull_no_in_pool_kind: u64,
    pub pull_no_in_banner: Option<u64>,
    pub pity_5_before: u64,
    pub pity_5_after: u64,
    pub pity_4_before: u64,
    pub pity_4_after: u64,
    pub hit_rarity: Option<u8>,
    pub rate_up_result: RateUpResult,
    pub result_confidence: String,
    pub guarantee_5_before: Option<bool>,
    pub guarantee_5_after: Option<bool>,
    pub guarantee_4_before: Option<bool>,
    pub guarantee_4_after: Option<bool>,
    pub rule: GachaRuleView,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DisplayRecord {
    pub record_id: String,
    pub record_type: String,
    pub time: Option<String>,
    pub pool_kind: PoolKind,
    pub pool_id: String,
    pub pool_label: String,
    pub banner: ResolvedBanner,
    pub item_id: String,
    pub item_name: String,
    pub item_asset_refs: BTreeMap<String, serde_json::Value>,
    pub rarity: Option<u8>,
    pub count: Option<i64>,
    pub roll_points: Option<i64>,
    pub secondary_item_id: Option<String>,
    pub secondary_item_name: Option<String>,
    pub secondary_item_asset_refs: BTreeMap<String, serde_json::Value>,
    pub secondary_count: Option<i64>,
    pub derived: RecordDerived,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RecordFilter {
    pub pool_kind: Option<PoolKind>,
    pub pool_id: Option<String>,
    pub banner_id: Option<String>,
    pub record_type: Option<String>,
    pub rarity: Option<u8>,
    pub hit_rarity: Option<u8>,
    pub rate_up_result: Option<RateUpResult>,
    pub pity_5_min: Option<u64>,
    pub pity_5_max: Option<u64>,
    pub pity_4_min: Option<u64>,
    pub pity_4_max: Option<u64>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub search: Option<String>,
    pub sort_key: Option<RecordSortKey>,
    pub sort_direction: Option<SortDirection>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordList {
    pub total: u64,
    pub records: Vec<DisplayRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordFilterOptions {
    pub pools: Vec<RecordPoolOption>,
    pub banners: Vec<RecordBannerOption>,
    pub record_types: Vec<RecordTypeOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordPoolOption {
    pub pool_id: String,
    pub pool_kind: PoolKind,
    pub label: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordBannerOption {
    pub banner_id: String,
    pub pool_kind: PoolKind,
    pub title: String,
    pub count: u64,
    pub phase: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordTypeOption {
    pub record_type: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MapLocaleList {
    pub locales: Vec<String>,
}
