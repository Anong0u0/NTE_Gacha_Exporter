impl JsonStore {
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
