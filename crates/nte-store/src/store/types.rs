use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use atomic_write_file::AtomicWriteFile;
use serde::{Deserialize, Serialize};
use zip::{ZipArchive, ZipWriter, write::FileOptions};

use nte_core::{compare_records_chronological, parse_public_document};
use nte_core::{
    BackupReport, DashboardOverview, DashboardSelection, DashboardSelectionDetail, GuiError,
    ImportReport, InternalRecord, PoolKind, PoolKindDetail, Profile, ProfileAnalysisView, RecordFilter,
    RecordFilterOptions, RecordList, RestoreReport, Settings, SettingsPatch,
};
use nte_core::{MapData, load_map};
use nte_core::{
    dashboard_overview, dashboard_selection_detail, list_records, pool_kind_detail,
    profile_analysis_view, record_filter_options,
};
use nte_core::{export_csv, export_public_json};
use nte_core::{assign_stable_record_ids, record_semantic_key, stable_record_id_from_key};

const DEFAULT_PROFILE: &str = "default";
const DEFAULT_LOCALE: &str = "en";
const DEFAULT_UI_LOCALE: &str = "en";
const DEFAULT_UPDATE_CHANNEL: &str = "stable";
const DEFAULT_CHECK_UPDATES_ON_STARTUP: bool = true;
static UNIQUE_STAMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct JsonStore {
    root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreDefaults {
    pub locale: String,
    pub ui_locale: String,
}

impl Default for StoreDefaults {
    fn default() -> Self {
        Self {
            locale: DEFAULT_LOCALE.to_string(),
            ui_locale: DEFAULT_UI_LOCALE.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataBackup {
    pub path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
struct DiskSettings {
    schema_version: u32,
    active_profile: String,
    #[serde(default = "default_locale")]
    locale: String,
    #[serde(default = "default_ui_locale")]
    ui_locale: String,
    #[serde(default = "default_update_channel")]
    update_channel: String,
    #[serde(default = "default_check_updates_on_startup")]
    check_updates_on_startup: bool,
    #[serde(default)]
    skipped_update_version: Option<String>,
    #[serde(default = "default_capture_auto_page_enabled")]
    capture_auto_page_enabled: bool,
    #[serde(default)]
    capture_full_update_enabled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct DiskProfile {
    schema_version: u32,
    name: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct DiskRecords {
    schema_version: u32,
    records: Vec<InternalRecord>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DiskLastRun {
    schema_version: u32,
    report: ImportReport,
}

#[derive(Debug, Deserialize)]
struct BackupManifest {
    schema: String,
    schema_version: u32,
    files: Vec<String>,
}

#[derive(Debug)]
struct BackupSnapshot {
    settings: DiskSettings,
    profiles: BTreeMap<String, SnapshotProfile>,
}

#[derive(Debug)]
struct SnapshotProfile {
    profile: DiskProfile,
    records: Vec<InternalRecord>,
    last_run: Option<ImportReport>,
}
