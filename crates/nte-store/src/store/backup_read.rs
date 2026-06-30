impl JsonStore {
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
        let mut settings: DiskSettings = read_zip_json(&mut zip, "settings.json")?;
        validate_locale(&settings.locale)?;
        settings.ui_locale =
            normalize_ui_locale_or_default(&settings.ui_locale, DEFAULT_UI_LOCALE)?;
        validate_update_channel(&settings.update_channel)?;
        if let Some(skipped_update_version) = settings.skipped_update_version.as_deref() {
            validate_update_version(skipped_update_version)?;
        }
        if settings.capture_full_update_enabled {
            settings.capture_auto_page_enabled = true;
        }
        if !settings.capture_auto_page_enabled {
            settings.capture_full_update_enabled = false;
        }

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
            let mut records: DiskRecords = read_zip_json(&mut zip, &records_path)?;
            let map = load_map(&settings.locale)?;
            validate_records_against_map(&records.records, &map)?;
            canonicalize_records_against_map(&mut records.records, &map);
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
        let map = load_map(&snapshot.settings.locale)?;

        for (snapshot_name, snapshot_profile) in &snapshot.profiles {
            records_seen += snapshot_profile.records.len() as u64;
            let existing = self.profile_name_case_insensitive(snapshot_name)?;
            let target_name = existing.unwrap_or_else(|| snapshot_name.clone());
            target_names.insert(snapshot_name.clone(), target_name.clone());
            if self.profile_dir(&target_name).exists() {
                profiles_merged += 1;
                let mut old_records = self.read_records(&target_name)?;
                canonicalize_records_against_map(&mut old_records, &map);
                let mut incoming_records = snapshot_profile.records.clone();
                canonicalize_records_against_map(&mut incoming_records, &map);
                let old_counts = semantic_counts(&old_records);
                let mut incoming_counts = BTreeMap::<String, u64>::new();
                let mut merged = old_records;
                for record in &incoming_records {
                    let key = record_semantic_key(record);
                    let occurrence = incoming_counts.entry(key.clone()).or_default();
                    let old_count = old_counts.get(&key).copied().unwrap_or_default();
                    if *occurrence < old_count {
                        records_skipped += 1;
                    } else {
                        let mut record = record.clone();
                        record.record_id = stable_record_id_from_key(&key, *occurrence);
                        records_inserted += 1;
                        merged.push(record);
                    }
                    *occurrence += 1;
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
}
