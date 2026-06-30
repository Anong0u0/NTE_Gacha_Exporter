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
    #[error("invalid assets pack: {0}")]
    InvalidAssetsPack(String),
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Settings {
    pub active_profile: String,
    pub locale: String,
    pub ui_locale: String,
    pub update_channel: String,
    pub check_updates_on_startup: bool,
    pub skipped_update_version: Option<String>,
    pub capture_auto_page_enabled: bool,
    pub capture_full_update_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SettingsPatch {
    pub active_profile: Option<String>,
    pub locale: Option<String>,
    pub ui_locale: Option<String>,
    pub update_channel: Option<String>,
    pub check_updates_on_startup: Option<bool>,
    pub skipped_update_version: Option<String>,
    pub capture_auto_page_enabled: Option<bool>,
    pub capture_full_update_enabled: Option<bool>,
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
    pub release_notes: String,
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
pub enum BannerResolutionIssue {
    UnknownPool,
    UnknownTime,
    OutsideKnownWindows,
    Ambiguous,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuleResolutionIssue {
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RollBucket {
    Gift,
    Sleep,
    #[serde(rename = "1")]
    One,
    #[serde(rename = "2")]
    Two,
    #[serde(rename = "3")]
    Three,
    #[serde(rename = "4")]
    Four,
    #[serde(rename = "5")]
    Five,
    #[serde(rename = "6")]
    Six,
    NotApplicable,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Character,
    Fork,
    Fashion,
    Glider,
    Inventory,
    VehicleModule,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolvedBanner {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution_issue: Option<BannerResolutionIssue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub banner_id: Option<String>,
    pub pool_id: Option<String>,
    pub pool_kind: Option<String>,
    pub banner_type: Option<String>,
    pub title: Option<String>,
    pub version: Option<String>,
    pub start_at: Option<String>,
    pub end_at: Option<String>,
    pub timezone: Option<String>,
    pub rate_up_5: Vec<String>,
    pub rate_up_4: Vec<String>,
    pub standard_5_pool: Vec<String>,
    pub standard_4_pool: Vec<String>,
    pub rule_id: Option<String>,
    pub asset_refs: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GachaRuleView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution_issue: Option<RuleResolutionIssue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub rule_id: Option<String>,
    pub pool_kind: PoolKind,
    pub hard_pity_5: Option<u64>,
    pub hard_up_pity_5: Option<u64>,
    pub pickup_win_rate_5: Option<u8>,
    pub has_guarantee_5: Option<bool>,
    pub guarantee_scope: Option<String>,
    pub carry_scope: Option<String>,
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

impl ItemKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Character => "character",
            Self::Fork => "fork",
            Self::Fashion => "fashion",
            Self::Glider => "glider",
            Self::Inventory => "inventory",
            Self::VehicleModule => "vehicle_module",
            Self::Unknown => "unknown",
        }
    }
}
