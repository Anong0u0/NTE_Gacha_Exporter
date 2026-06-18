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

impl JsonStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, GuiError> {
        let store = Self {
            root: root.as_ref().to_path_buf(),
        };
        store.bootstrap()?;
        Ok(store)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn settings(&self) -> Result<Settings, GuiError> {
        let settings = self.read_settings()?;
        Ok(Settings {
            active_profile: settings.active_profile,
            locale: settings.locale,
            update_channel: settings.update_channel,
            check_updates_on_startup: settings.check_updates_on_startup,
        })
    }

    pub fn update_settings(&self, patch: SettingsPatch) -> Result<Settings, GuiError> {
        let mut settings = self.read_settings()?;
        if let Some(active_profile) = patch.active_profile {
            let active_profile = validate_profile_name(&active_profile)?;
            self.read_profile(&active_profile)?;
            settings.active_profile = active_profile;
        }
        if let Some(locale) = patch.locale {
            validate_locale(&locale)?;
            settings.locale = locale;
        }
        if let Some(update_channel) = patch.update_channel {
            settings.update_channel = validate_update_channel(&update_channel)?;
        }
        if let Some(check_updates_on_startup) = patch.check_updates_on_startup {
            settings.check_updates_on_startup = check_updates_on_startup;
        }
        self.write_settings(&settings)?;
        self.settings()
    }

    pub fn list_profiles(&self) -> Result<Vec<Profile>, GuiError> {
        let active_profile = self.read_settings()?.active_profile;
        let profiles_dir = self.profiles_dir();
        let mut profiles = Vec::new();
        if profiles_dir.exists() {
            for entry in fs::read_dir(profiles_dir)? {
                let entry = entry?;
                if !entry.file_type()?.is_dir() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().to_string();
                let profile = self.read_profile(&name)?;
                profiles.push(Profile {
                    active: profile.name == active_profile,
                    name: profile.name,
                    created_at: profile.created_at,
                    updated_at: profile.updated_at,
                });
            }
        }
        profiles.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(profiles)
    }

    pub fn create_profile(&self, name: &str) -> Result<Profile, GuiError> {
        let name = validate_profile_name(name)?;
        self.ensure_profile_absent(&name)?;
        let now = now_stamp();
        fs::create_dir_all(self.profile_dir(&name))?;
        let profile = DiskProfile {
            schema_version: 1,
            name: name.clone(),
            created_at: now.clone(),
            updated_at: now,
        };
        self.write_profile(&profile)?;
        self.write_records(&name, &[])?;
        Ok(Profile {
            name,
            created_at: profile.created_at,
            updated_at: profile.updated_at,
            active: false,
        })
    }

    pub fn set_active_profile(&self, name: &str) -> Result<Settings, GuiError> {
        self.update_settings(SettingsPatch {
            active_profile: Some(name.to_string()),
            ..SettingsPatch::default()
        })
    }

    pub fn import_public_document(
        &self,
        profile_name: &str,
        document_text: &str,
        source_kind: &str,
        source_path: Option<&str>,
    ) -> Result<ImportReport, GuiError> {
        let profile_name = validate_profile_name(profile_name)?;
        self.read_profile(&profile_name)?;
        let incoming = parse_public_document(document_text)?;
        let map = load_map(&self.read_settings()?.locale)?;
        validate_records_against_map(&incoming, &map)?;
        self.merge_records(&profile_name, incoming, source_kind, source_path)
    }

    pub fn import_public_document_with_backup(
        &self,
        profile_name: &str,
        document_text: &str,
        source_kind: &str,
        source_path: Option<&str>,
    ) -> Result<ImportReport, GuiError> {
        let backup = self.create_data_backup()?;
        match self.import_public_document(profile_name, document_text, source_kind, source_path) {
            Ok(report) => Ok(report),
            Err(error) => {
                let _ = self.replace_data_from_backup(&backup);
                Err(error)
            }
        }
    }

    pub fn profile_record_ids(&self, profile_name: &str) -> Result<Vec<String>, GuiError> {
        let profile_name = validate_profile_name(profile_name)?;
        self.read_profile(&profile_name)?;
        Ok(self
            .read_records(&profile_name)?
            .into_iter()
            .map(|record| record.record_id)
            .collect())
    }

    pub fn default_run_raw_path(&self) -> PathBuf {
        self.root
            .join("data/runs")
            .join(format!("raw-{}.jsonl", now_unique_stamp()))
    }

    pub fn create_data_backup(&self) -> Result<DataBackup, GuiError> {
        let path = self
            .root
            .join("data/backups")
            .join(format!("backup-{}.zip", now_unique_stamp()));
        self.create_data_backup_at(path)
    }

    pub fn create_data_backup_at(&self, path: impl AsRef<Path>) -> Result<DataBackup, GuiError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = fs::File::create(&path)?;
        let mut zip = ZipWriter::new(file);
        let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let mut files = Vec::new();
        self.add_backup_file(&mut zip, options, Path::new("settings.json"), &mut files)?;
        self.add_backup_dir(&mut zip, options, Path::new("profiles"), &mut files)?;
        let manifest = serde_json::json!({
            "schema": "nte-gacha-exporter-data-backup",
            "schema_version": 1,
            "created_at": now_stamp(),
            "files": files,
        });
        zip.start_file("manifest.json", options)?;
        zip.write_all(&serde_json::to_vec_pretty(&manifest)?)?;
        zip.write_all(b"\n")?;
        zip.finish()?;
        Ok(DataBackup { path })
    }

    pub fn create_data_backup_report(
        &self,
        path: Option<impl AsRef<Path>>,
    ) -> Result<BackupReport, GuiError> {
        let created_at = now_stamp();
        let backup = match path {
            Some(path) => self.create_data_backup_at(path)?,
            None => self.create_data_backup()?,
        };
        let profiles = self.list_profiles()?;
        let record_count = profiles.iter().try_fold(0_u64, |count, profile| {
            Ok::<u64, GuiError>(count + self.read_records(&profile.name)?.len() as u64)
        })?;
        Ok(BackupReport {
            path: backup.path.to_string_lossy().to_string(),
            profile_count: profiles.len() as u64,
            record_count,
            created_at,
        })
    }

    pub fn restore_data_backup(&self, backup: &DataBackup) -> Result<(), GuiError> {
        self.restore_data_backup_report(&backup.path).map(|_| ())
    }

    pub fn restore_data_backup_report(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<RestoreReport, GuiError> {
        let source_path = path.as_ref().to_string_lossy().to_string();
        let snapshot = self.read_backup_snapshot(path.as_ref())?;
        let rollback = self.create_data_backup()?;
        match self.apply_backup_snapshot(snapshot, &source_path) {
            Ok(report) => Ok(report),
            Err(error) => {
                let _ = self.replace_data_from_backup(&rollback);
                Err(error)
            }
        }
    }

    pub fn dashboard_overview(
        &self,
        profile_name: &str,
        locale: &str,
    ) -> Result<DashboardOverview, GuiError> {
        let profile = self.profile_for_api(profile_name)?;
        let map = load_map(locale)?;
        let records = self.read_records(&profile.name)?;
        let last_run = self.read_last_run(&profile.name)?;
        dashboard_overview(profile, last_run, &records, &map)
    }

    pub fn pool_kind_detail(
        &self,
        profile_name: &str,
        locale: &str,
        pool_kind: PoolKind,
    ) -> Result<PoolKindDetail, GuiError> {
        let profile = self.profile_for_api(profile_name)?;
        let map = load_map(locale)?;
        let records = self.read_records(&profile.name)?;
        pool_kind_detail(&records, &map, pool_kind)
    }

    pub fn list_records(
        &self,
        profile_name: &str,
        locale: &str,
        filter: &RecordFilter,
    ) -> Result<RecordList, GuiError> {
        let profile = self.profile_for_api(profile_name)?;
        let map = load_map(locale)?;
        let records = self.read_records(&profile.name)?;
        list_records(&records, &map, filter)
    }

    pub fn record_filter_options(
        &self,
        profile_name: &str,
        locale: &str,
    ) -> Result<RecordFilterOptions, GuiError> {
        let profile = self.profile_for_api(profile_name)?;
        let map = load_map(locale)?;
        let records = self.read_records(&profile.name)?;
        record_filter_options(&records, &map)
    }

    pub fn export_public_json(
        &self,
        profile_name: &str,
        locale: &str,
        path: impl AsRef<Path>,
    ) -> Result<(), GuiError> {
        let profile = self.profile_for_api(profile_name)?;
        let map = load_map(locale)?;
        let records = self.read_records(&profile.name)?;
        export_public_json(path.as_ref(), &records, &map, locale)
    }

    pub fn export_csv(
        &self,
        profile_name: &str,
        locale: &str,
        path: impl AsRef<Path>,
    ) -> Result<(), GuiError> {
        let profile = self.profile_for_api(profile_name)?;
        let map = load_map(locale)?;
        let records = self.read_records(&profile.name)?;
        export_csv(path.as_ref(), &records, &map)
    }

    fn bootstrap(&self) -> Result<(), GuiError> {
        fs::create_dir_all(self.root.join("data"))?;
        fs::create_dir_all(self.profiles_dir())?;
        fs::create_dir_all(self.root.join("data/backups"))?;
        fs::create_dir_all(self.root.join("data/runs"))?;
        if !self.settings_path().exists() {
            self.write_settings(&DiskSettings {
                schema_version: 1,
                active_profile: DEFAULT_PROFILE.to_string(),
                locale: DEFAULT_LOCALE.to_string(),
                update_channel: DEFAULT_UPDATE_CHANNEL.to_string(),
                check_updates_on_startup: false,
            })?;
        }
        if !self.profile_dir(DEFAULT_PROFILE).exists() {
            self.create_profile(DEFAULT_PROFILE)?;
        }
        Ok(())
    }

    fn merge_records(
        &self,
        profile_name: &str,
        mut incoming: Vec<InternalRecord>,
        source_kind: &str,
        source_path: Option<&str>,
    ) -> Result<ImportReport, GuiError> {
        let old_records = self.read_records(profile_name)?;
        let old_last_run = self.read_last_run(profile_name)?;
        let mut seen_ids: HashSet<String> = old_records
            .iter()
            .map(|record| record.record_id.clone())
            .collect();
        let mut merged = old_records.clone();
        let mut inserted = 0_u64;
        let mut skipped = 0_u64;

        for record in incoming.drain(..) {
            if seen_ids.insert(record.record_id.clone()) {
                inserted += 1;
                merged.push(record);
            } else {
                skipped += 1;
            }
        }
        sort_records(&mut merged);

        let report = ImportReport {
            profile_name: profile_name.to_string(),
            source_kind: source_kind.to_string(),
            source_path: source_path.map(str::to_string),
            records_seen: inserted + skipped,
            records_inserted: inserted,
            records_skipped: skipped,
            completed_at: now_stamp(),
        };

        if let Err(error) = self.write_records(profile_name, &merged).and_then(|()| {
            self.write_last_run(profile_name, &report)?;
            self.touch_profile(profile_name)
        }) {
            let _ = self.write_records(profile_name, &old_records);
            match old_last_run {
                Some(old) => {
                    let _ = self.write_last_run(profile_name, &old);
                }
                None => {
                    let _ = fs::remove_file(self.last_run_path(profile_name));
                }
            }
            return Err(error);
        }
        Ok(report)
    }

    fn profile_for_api(&self, profile_name: &str) -> Result<Profile, GuiError> {
        let profile_name = validate_profile_name(profile_name)?;
        let active_profile = self.read_settings()?.active_profile;
        let profile = self.read_profile(&profile_name)?;
        Ok(Profile {
            active: profile.name == active_profile,
            name: profile.name,
            created_at: profile.created_at,
            updated_at: profile.updated_at,
        })
    }

    fn ensure_profile_absent(&self, name: &str) -> Result<(), GuiError> {
        let lower = name.to_ascii_lowercase();
        for profile in self.list_profiles()? {
            if profile.name.to_ascii_lowercase() == lower {
                return Err(GuiError::InvalidProfile(format!(
                    "profile already exists: {name}"
                )));
            }
        }
        Ok(())
    }

    fn read_backup_snapshot(&self, path: &Path) -> Result<BackupSnapshot, GuiError> {
        let file = fs::File::open(path)?;
        let mut zip = ZipArchive::new(file)?;
        let manifest: BackupManifest = read_zip_json(&mut zip, "manifest.json")?;
        if manifest.schema != "nte-gacha-exporter-data-backup" || manifest.schema_version != 1 {
            return Err(GuiError::InvalidBackup(
                "manifest schema must be nte-gacha-exporter-data-backup v1".to_string(),
            ));
        }
        let files = manifest.files.into_iter().collect::<HashSet<_>>();
        validate_backup_files(&files)?;
        if !files.contains("settings.json") {
            return Err(GuiError::InvalidBackup(
                "manifest missing settings.json".to_string(),
            ));
        }
        let entry_names = backup_entry_names(&mut zip)?;
        validate_backup_files(&entry_names)?;
        for name in entry_names
            .iter()
            .filter(|name| name.as_str() != "manifest.json")
        {
            if !files.contains(name) {
                return Err(GuiError::InvalidBackup(format!(
                    "backup entry not listed in manifest: {name}"
                )));
            }
        }
        for name in &files {
            if !entry_names.contains(name) {
                return Err(GuiError::InvalidBackup(format!(
                    "manifest file missing from zip: {name}"
                )));
            }
        }
        let settings: DiskSettings = read_zip_json(&mut zip, "settings.json")?;
        validate_locale(&settings.locale)?;
        validate_update_channel(&settings.update_channel)?;

        let mut profiles = BTreeMap::new();
        for name in backup_profile_names(&files)? {
            let profile_path = format!("profiles/{name}/profile.json");
            let records_path = format!("profiles/{name}/records.json");
            if !files.contains(&profile_path) || !files.contains(&records_path) {
                return Err(GuiError::InvalidBackup(format!(
                    "profile {name} missing profile.json or records.json"
                )));
            }
            let profile: DiskProfile = read_zip_json(&mut zip, &profile_path)?;
            let canonical_name = validate_profile_name(&profile.name)?;
            if canonical_name != name {
                return Err(GuiError::InvalidBackup(format!(
                    "profile path/name mismatch: {name}"
                )));
            }
            if profiles
                .keys()
                .any(|existing: &String| existing.eq_ignore_ascii_case(&canonical_name))
            {
                return Err(GuiError::InvalidBackup(format!(
                    "duplicate profile name: {canonical_name}"
                )));
            }
            let records: DiskRecords = read_zip_json(&mut zip, &records_path)?;
            validate_records_against_map(&records.records, &load_map(&settings.locale)?)?;
            let last_run_path = format!("profiles/{name}/last-run.json");
            let last_run = if files.contains(&last_run_path) {
                Some(read_zip_json::<DiskLastRun>(&mut zip, &last_run_path)?.report)
            } else {
                None
            };
            profiles.insert(
                canonical_name,
                SnapshotProfile {
                    profile,
                    records: records.records,
                    last_run,
                },
            );
        }
        if !profiles.contains_key(&settings.active_profile) {
            return Err(GuiError::InvalidBackup(format!(
                "active profile not found in snapshot: {}",
                settings.active_profile
            )));
        }
        Ok(BackupSnapshot { settings, profiles })
    }

    fn apply_backup_snapshot(
        &self,
        mut snapshot: BackupSnapshot,
        source_path: &str,
    ) -> Result<RestoreReport, GuiError> {
        let mut profiles_created = 0_u64;
        let mut profiles_merged = 0_u64;
        let mut records_seen = 0_u64;
        let mut records_inserted = 0_u64;
        let mut records_skipped = 0_u64;
        let completed_at = now_stamp();
        let mut target_names = BTreeMap::new();

        for (snapshot_name, snapshot_profile) in &snapshot.profiles {
            records_seen += snapshot_profile.records.len() as u64;
            let existing = self.profile_name_case_insensitive(snapshot_name)?;
            let target_name = existing.unwrap_or_else(|| snapshot_name.clone());
            target_names.insert(snapshot_name.clone(), target_name.clone());
            if self.profile_dir(&target_name).exists() {
                profiles_merged += 1;
                let old_records = self.read_records(&target_name)?;
                let mut seen_ids = old_records
                    .iter()
                    .map(|record| record.record_id.clone())
                    .collect::<HashSet<_>>();
                let mut merged = old_records;
                for record in &snapshot_profile.records {
                    if seen_ids.insert(record.record_id.clone()) {
                        records_inserted += 1;
                        merged.push(record.clone());
                    } else {
                        records_skipped += 1;
                    }
                }
                sort_records(&mut merged);
                self.write_records(&target_name, &merged)?;
                if let Some(last_run) = snapshot_profile.last_run.as_ref() {
                    self.write_last_run(&target_name, last_run)?;
                }
                self.touch_profile(&target_name)?;
            } else {
                profiles_created += 1;
                fs::create_dir_all(self.profile_dir(&target_name))?;
                self.write_profile(&snapshot_profile.profile)?;
                self.write_records(&target_name, &snapshot_profile.records)?;
                if let Some(last_run) = snapshot_profile.last_run.as_ref() {
                    self.write_last_run(&target_name, last_run)?;
                }
                records_inserted += snapshot_profile.records.len() as u64;
            }
        }
        if let Some(active_profile) = target_names.get(&snapshot.settings.active_profile) {
            snapshot.settings.active_profile = active_profile.clone();
        }
        self.write_settings(&snapshot.settings)?;
        Ok(RestoreReport {
            source_path: source_path.to_string(),
            profiles_seen: snapshot.profiles.len() as u64,
            profiles_created,
            profiles_merged,
            records_seen,
            records_inserted,
            records_skipped,
            settings_restored: true,
            completed_at,
        })
    }

    fn replace_data_from_backup(&self, backup: &DataBackup) -> Result<(), GuiError> {
        let snapshot = self.read_backup_snapshot(&backup.path)?;
        if self.settings_path().exists() {
            fs::remove_file(self.settings_path())?;
        }
        if self.profiles_dir().exists() {
            for entry in fs::read_dir(self.profiles_dir())? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    remove_profile_dir_known_files(entry.path())?;
                }
            }
        }
        fs::create_dir_all(self.profiles_dir())?;
        for (name, profile) in snapshot.profiles {
            fs::create_dir_all(self.profile_dir(&name))?;
            self.write_profile(&profile.profile)?;
            self.write_records(&name, &profile.records)?;
            if let Some(last_run) = profile.last_run.as_ref() {
                self.write_last_run(&name, last_run)?;
            }
        }
        self.write_settings(&snapshot.settings)?;
        Ok(())
    }

    fn profile_name_case_insensitive(&self, name: &str) -> Result<Option<String>, GuiError> {
        let lower = name.to_ascii_lowercase();
        for profile in self.list_profiles()? {
            if profile.name.to_ascii_lowercase() == lower {
                return Ok(Some(profile.name));
            }
        }
        Ok(None)
    }

    fn read_settings(&self) -> Result<DiskSettings, GuiError> {
        read_json(&self.settings_path())
    }

    fn write_settings(&self, settings: &DiskSettings) -> Result<(), GuiError> {
        write_json(&self.settings_path(), settings)
    }

    fn read_profile(&self, name: &str) -> Result<DiskProfile, GuiError> {
        let path = self.profile_path(name);
        if !path.exists() {
            return Err(GuiError::ProfileNotFound(name.to_string()));
        }
        read_json(&path)
    }

    fn write_profile(&self, profile: &DiskProfile) -> Result<(), GuiError> {
        write_json(&self.profile_path(&profile.name), profile)
    }

    fn touch_profile(&self, name: &str) -> Result<(), GuiError> {
        let mut profile = self.read_profile(name)?;
        profile.updated_at = now_stamp();
        self.write_profile(&profile)
    }

    fn read_records(&self, profile_name: &str) -> Result<Vec<InternalRecord>, GuiError> {
        let path = self.records_path(profile_name);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let mut disk: DiskRecords = read_json(&path)?;
        normalize_records(&mut disk.records);
        sort_records(&mut disk.records);
        Ok(disk.records)
    }

    fn write_records(
        &self,
        profile_name: &str,
        records: &[InternalRecord],
    ) -> Result<(), GuiError> {
        let mut records = records.to_vec();
        normalize_records(&mut records);
        write_json(
            &self.records_path(profile_name),
            &DiskRecords {
                schema_version: 1,
                records,
            },
        )
    }

    fn read_last_run(&self, profile_name: &str) -> Result<Option<ImportReport>, GuiError> {
        let path = self.last_run_path(profile_name);
        if !path.exists() {
            return Ok(None);
        }
        let disk: DiskLastRun = read_json(&path)?;
        Ok(Some(disk.report))
    }

    fn write_last_run(&self, profile_name: &str, report: &ImportReport) -> Result<(), GuiError> {
        write_json(
            &self.last_run_path(profile_name),
            &DiskLastRun {
                schema_version: 1,
                report: report.clone(),
            },
        )
    }

    fn settings_path(&self) -> PathBuf {
        self.root.join("data/settings.json")
    }

    fn profiles_dir(&self) -> PathBuf {
        self.root.join("data/profiles")
    }

    fn profile_dir(&self, name: &str) -> PathBuf {
        self.profiles_dir().join(name)
    }

    fn profile_path(&self, name: &str) -> PathBuf {
        self.profile_dir(name).join("profile.json")
    }

    fn records_path(&self, name: &str) -> PathBuf {
        self.profile_dir(name).join("records.json")
    }

    fn last_run_path(&self, name: &str) -> PathBuf {
        self.profile_dir(name).join("last-run.json")
    }

    fn add_backup_dir(
        &self,
        zip: &mut ZipWriter<fs::File>,
        options: FileOptions,
        relative_dir: &Path,
        files: &mut Vec<String>,
    ) -> Result<(), GuiError> {
        let dir = self.root.join("data").join(relative_dir);
        if !dir.exists() {
            return Ok(());
        }
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let name = entry.file_name();
            let relative = relative_dir.join(name);
            if file_type.is_dir() {
                self.add_backup_dir(zip, options, &relative, files)?;
            } else if file_type.is_file() {
                self.add_backup_file(zip, options, &relative, files)?;
            }
        }
        Ok(())
    }

    fn add_backup_file(
        &self,
        zip: &mut ZipWriter<fs::File>,
        options: FileOptions,
        relative: &Path,
        files: &mut Vec<String>,
    ) -> Result<(), GuiError> {
        let path = self.root.join("data").join(relative);
        if !path.is_file() {
            return Ok(());
        }
        let name = relative.to_string_lossy().replace('\\', "/");
        zip.start_file(&name, options)?;
        let bytes = fs::read(path)?;
        zip.write_all(&bytes)?;
        files.push(name);
        Ok(())
    }
}

pub fn load_locale_or_settings(
    store: &JsonStore,
    locale: Option<String>,
) -> Result<String, GuiError> {
    match locale {
        Some(locale) if !locale.trim().is_empty() => Ok(locale),
        _ => Ok(store.settings()?.locale),
    }
}

fn validate_profile_name(name: &str) -> Result<String, GuiError> {
    let name = name.trim();
    if name.is_empty() || name.len() > 40 {
        return Err(GuiError::InvalidProfile(
            "profile name length must be 1..40".to_string(),
        ));
    }
    if !name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(GuiError::InvalidProfile(
            "profile name must use ASCII letters, digits, _ or -".to_string(),
        ));
    }
    if is_reserved_windows_name(name) {
        return Err(GuiError::InvalidProfile(
            "profile name must not use a reserved Windows device name".to_string(),
        ));
    }
    Ok(name.to_string())
}

fn is_reserved_windows_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || upper
            .strip_prefix("COM")
            .and_then(|tail| tail.parse::<u8>().ok())
            .is_some_and(|value| (1..=9).contains(&value))
        || upper
            .strip_prefix("LPT")
            .and_then(|tail| tail.parse::<u8>().ok())
            .is_some_and(|value| (1..=9).contains(&value))
}

fn validate_locale(locale: &str) -> Result<(), GuiError> {
    load_map(locale).map(|_| ())
}

fn validate_update_channel(channel: &str) -> Result<String, GuiError> {
    let channel = channel.trim();
    if channel.is_empty() || channel.len() > 32 {
        return Err(GuiError::InvalidDocument(
            "update_channel length must be 1..32".to_string(),
        ));
    }
    if !channel
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(GuiError::InvalidDocument(
            "update_channel must use ASCII letters, digits, _ or -".to_string(),
        ));
    }
    Ok(channel.to_string())
}

fn validate_records_against_map(records: &[InternalRecord], map: &MapData) -> Result<(), GuiError> {
    for record in records {
        if !map.has_pool_id(&record.pool_id) {
            return Err(GuiError::UnknownPoolId(record.pool_id.clone()));
        }
    }
    Ok(())
}

fn normalize_records(records: &mut [InternalRecord]) {
    for record in records {
        if record
            .roll_points
            .is_some_and(|value| matches!(value, 0 | 4_294_967_295))
        {
            record.roll_points = None;
        }
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, GuiError> {
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn read_zip_json<T: for<'de> Deserialize<'de>>(
    zip: &mut ZipArchive<fs::File>,
    name: &str,
) -> Result<T, GuiError> {
    let mut entry = zip
        .by_name(name)
        .map_err(|_| GuiError::InvalidBackup(format!("backup missing required file: {name}")))?;
    if !entry.is_file() {
        return Err(GuiError::InvalidBackup(format!(
            "backup entry must be a file: {name}"
        )));
    }
    let mut text = String::new();
    entry.read_to_string(&mut text)?;
    Ok(serde_json::from_str(&text)?)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), GuiError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = AtomicWriteFile::open(path)?;
    let bytes = serde_json::to_vec_pretty(value)?;
    file.write_all(&bytes)?;
    file.write_all(b"\n")?;
    file.commit()?;
    Ok(())
}

fn validate_backup_files(files: &HashSet<String>) -> Result<(), GuiError> {
    for name in files {
        if name.is_empty()
            || name == "."
            || name.contains('\\')
            || name.contains("..")
            || name.starts_with('/')
        {
            return Err(GuiError::InvalidBackup(format!(
                "invalid backup path: {name}"
            )));
        }
        if name != "settings.json"
            && !is_supported_profile_backup_path(name)
            && name != "manifest.json"
        {
            return Err(GuiError::InvalidBackup(format!(
                "unsupported backup path: {name}"
            )));
        }
    }
    Ok(())
}

fn backup_profile_names(files: &HashSet<String>) -> Result<Vec<String>, GuiError> {
    let mut names = Vec::new();
    for name in files {
        let Some(rest) = name.strip_prefix("profiles/") else {
            continue;
        };
        let mut parts = rest.split('/');
        let Some(profile_name) = parts.next() else {
            continue;
        };
        if profile_name.is_empty() {
            return Err(GuiError::InvalidBackup(format!(
                "invalid backup path: {name}"
            )));
        }
        validate_profile_name(profile_name)?;
        if parts.next().is_none() {
            return Err(GuiError::InvalidBackup(format!(
                "profile path missing filename: {name}"
            )));
        }
        if !names
            .iter()
            .any(|existing: &String| existing == profile_name)
        {
            names.push(profile_name.to_string());
        }
    }
    names.sort();
    Ok(names)
}

fn is_supported_profile_backup_path(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("profiles/") else {
        return false;
    };
    let mut parts = rest.split('/');
    let Some(profile_name) = parts.next() else {
        return false;
    };
    let Some(file_name) = parts.next() else {
        return false;
    };
    parts.next().is_none()
        && !profile_name.is_empty()
        && matches!(file_name, "profile.json" | "records.json" | "last-run.json")
}

fn remove_profile_dir_known_files(path: PathBuf) -> Result<(), GuiError> {
    for file_name in ["profile.json", "records.json", "last-run.json"] {
        let file = path.join(file_name);
        if file.exists() {
            fs::remove_file(file)?;
        }
    }
    let _ = fs::remove_dir(path);
    Ok(())
}

fn backup_entry_names(zip: &mut ZipArchive<fs::File>) -> Result<HashSet<String>, GuiError> {
    let mut names = HashSet::new();
    for index in 0..zip.len() {
        let entry = zip.by_index(index)?;
        if entry.is_file() {
            names.insert(entry.name().to_string());
        }
    }
    Ok(names)
}

fn sort_records(records: &mut [InternalRecord]) {
    records.sort_by(|left, right| {
        left.time
            .cmp(&right.time)
            .then_with(|| left.record_id.cmp(&right.record_id))
    });
}

fn now_stamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn now_unique_stamp() -> String {
    let sequence = UNIQUE_STAMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| format!("{}-{:09}-{sequence}", value.as_secs(), value.subsec_nanos()))
        .unwrap_or_else(|_| format!("0-000000000-{sequence}"))
}

fn default_update_channel() -> String {
    DEFAULT_UPDATE_CHANNEL.to_string()
}
