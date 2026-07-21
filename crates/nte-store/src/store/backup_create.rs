impl JsonStore {
    fn create_generated_data_backup(&self) -> Result<DataBackup, GuiError> {
        let path = self
            .root
            .join("data/backups")
            .join(format!("backup-{}.zip", now_unique_stamp()));
        self.create_data_backup_at(path)
    }

    pub fn create_data_backup(&self) -> Result<DataBackup, GuiError> {
        let backup = self.create_generated_data_backup()?;
        self.cleanup_generated_backups_keep_latest()?;
        Ok(backup)
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
            "schema_version": 2,
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
        let (backup, cleanup_generated) = match path {
            Some(path) => (self.create_data_backup_at(path)?, false),
            None => (self.create_generated_data_backup()?, true),
        };
        let profiles = self.list_profiles()?;
        let record_count = profiles.iter().try_fold(0_u64, |count, profile| {
            Ok::<u64, GuiError>(count + self.read_records(&profile.name)?.len() as u64)
        })?;
        let report = BackupReport {
            path: backup.path.to_string_lossy().to_string(),
            profile_count: profiles.len() as u64,
            record_count,
            created_at,
        };
        if cleanup_generated {
            self.cleanup_generated_backups_keep_latest()?;
        }
        Ok(report)
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
        let rollback = self.create_generated_data_backup()?;
        match self.apply_backup_snapshot(snapshot, &source_path) {
            Ok(report) => {
                self.cleanup_generated_backups_keep_latest()?;
                Ok(report)
            }
            Err(error) => {
                let _ = self.replace_data_from_backup(&rollback);
                Err(error)
            }
        }
    }
}
