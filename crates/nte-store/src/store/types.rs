use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use atomic_write_file::AtomicWriteFile;
use serde::{Deserialize, Serialize};
use zip::{ZipArchive, ZipWriter, write::FileOptions};

use nte_core::parse_public_document;
use nte_core::{
    BackupReport, DashboardOverview, GuiError, ImportReport, InternalRecord, PoolKind,
    PoolKindDetail, Profile, RecordFilter, RecordFilterOptions, RecordList, RestoreReport,
    Settings, SettingsPatch,
};
use nte_core::{MapData, load_map};
use nte_core::{dashboard_overview, list_records, pool_kind_detail, record_filter_options};
use nte_core::{export_csv, export_public_json};

const DEFAULT_PROFILE: &str = "default";
const DEFAULT_LOCALE: &str = "zh-Hant";
const DEFAULT_UPDATE_CHANNEL: &str = "stable";
static UNIQUE_STAMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct JsonStore {
    root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataBackup {
    pub path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
struct DiskSettings {
    schema_version: u32,
    active_profile: String,
    locale: String,
    #[serde(default = "default_update_channel")]
    update_channel: String,
    #[serde(default)]
    check_updates_on_startup: bool,
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

