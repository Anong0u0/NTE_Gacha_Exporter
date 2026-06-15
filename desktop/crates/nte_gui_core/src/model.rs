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
    pub rarity_distribution: Vec<RarityBucket>,
    pub item_ranking: Vec<ItemRank>,
    pub latest_records: Vec<DisplayRecord>,
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
    pub observed_up_rate: Option<f64>,
    pub latest_5star: Option<DisplayRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PoolKindDetail {
    pub summary: PoolKindSummary,
    pub five_star_history: Vec<FiveStarRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FiveStarRecord {
    pub record: DisplayRecord,
    pub pity_distance: u64,
    pub result: FiveStarResult,
    pub guarantee_before: bool,
    pub guarantee_after: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FiveStarResult {
    Up,
    OffRate,
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
pub struct DisplayRecord {
    pub record_id: String,
    pub record_type: String,
    pub time: Option<String>,
    pub pool_kind: PoolKind,
    pub pool_id: String,
    pub pool_label: String,
    pub item_id: String,
    pub item_name: String,
    pub rarity: Option<u8>,
    pub count: Option<i64>,
    pub roll_points: Option<i64>,
    pub secondary_item_id: Option<String>,
    pub secondary_item_name: Option<String>,
    pub secondary_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RecordFilter {
    pub pool_kind: Option<PoolKind>,
    pub pool_id: Option<String>,
    pub record_type: Option<String>,
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
pub struct RecordTypeOption {
    pub record_type: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MapLocaleList {
    pub locales: Vec<String>,
}
