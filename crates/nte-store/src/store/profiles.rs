impl JsonStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, GuiError> {
        Self::open_with_defaults(root, StoreDefaults::default())
    }

    pub fn open_with_defaults(
        root: impl AsRef<Path>,
        defaults: StoreDefaults,
    ) -> Result<Self, GuiError> {
        let store = Self {
            root: root.as_ref().to_path_buf(),
        };
        store.bootstrap(&defaults)?;
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
            ui_locale: settings.ui_locale,
            update_channel: settings.update_channel,
            check_updates_on_startup: settings.check_updates_on_startup,
            skipped_update_version: settings.skipped_update_version,
            capture_auto_page_enabled: settings.capture_auto_page_enabled,
            capture_full_update_enabled: settings.capture_full_update_enabled,
            capture_windivert_backend_enabled: settings.capture_windivert_backend_enabled,
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
        if let Some(ui_locale) = patch.ui_locale {
            settings.ui_locale = validate_ui_locale(&ui_locale)?;
        }
        if let Some(update_channel) = patch.update_channel {
            settings.update_channel = validate_update_channel(&update_channel)?;
        }
        if let Some(check_updates_on_startup) = patch.check_updates_on_startup {
            settings.check_updates_on_startup = check_updates_on_startup;
        }
        if let Some(skipped_update_version) = patch.skipped_update_version {
            settings.skipped_update_version =
                Some(validate_update_version(&skipped_update_version)?);
        }
        if let Some(capture_auto_page_enabled) = patch.capture_auto_page_enabled {
            settings.capture_auto_page_enabled = capture_auto_page_enabled;
            if !capture_auto_page_enabled {
                settings.capture_full_update_enabled = false;
            }
        }
        if let Some(capture_full_update_enabled) = patch.capture_full_update_enabled {
            settings.capture_full_update_enabled = capture_full_update_enabled;
            if capture_full_update_enabled {
                settings.capture_auto_page_enabled = true;
            }
        }
        if let Some(capture_windivert_backend_enabled) = patch.capture_windivert_backend_enabled {
            settings.capture_windivert_backend_enabled = capture_windivert_backend_enabled;
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

    pub fn rename_profile(&self, old_name: &str, new_name: &str) -> Result<Profile, GuiError> {
        let old_name = validate_profile_name(old_name)?;
        let new_name = validate_profile_name(new_name)?;
        if old_name == new_name {
            return self.profile_for_api(&old_name);
        }
        self.read_profile(&old_name)?;
        self.ensure_profile_absent(&new_name)?;

        let mut settings = self.read_settings()?;
        fs::rename(self.profile_dir(&old_name), self.profile_dir(&new_name))?;
        let mut profile = self.read_profile(&new_name)?;
        profile.name = new_name.clone();
        profile.updated_at = now_stamp();
        self.write_profile(&profile)?;
        if settings.active_profile == old_name {
            settings.active_profile = new_name.clone();
            self.write_settings(&settings)?;
        }
        self.profile_for_api(&new_name)
    }

    pub fn delete_profile(&self, name: &str) -> Result<Settings, GuiError> {
        let name = validate_profile_name(name)?;
        self.read_profile(&name)?;
        let profiles = self.list_profiles()?;
        if profiles.len() <= 1 {
            return Err(GuiError::InvalidProfile(
                "cannot delete the last profile".to_string(),
            ));
        }

        let mut settings = self.read_settings()?;
        let replacement = profiles
            .iter()
            .find(|profile| profile.name != name)
            .map(|profile| profile.name.clone())
            .ok_or_else(|| {
                GuiError::InvalidProfile("cannot delete the last profile".to_string())
            })?;
        remove_profile_dir_known_files_strict(self.profile_dir(&name))?;
        if settings.active_profile == name {
            settings.active_profile = replacement;
            self.write_settings(&settings)?;
        }
        self.settings()
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
        let mut incoming = parse_public_document(document_text)?;
        let map = load_map(&self.read_settings()?.locale)?;
        validate_records_against_map(&incoming, &map)?;
        canonicalize_records_against_map(&mut incoming, &map);
        self.merge_records(&profile_name, incoming, source_kind, source_path, &map)
    }

    pub fn import_public_document_with_backup(
        &self,
        profile_name: &str,
        document_text: &str,
        source_kind: &str,
        source_path: Option<&str>,
    ) -> Result<ImportReport, GuiError> {
        let backup = self.create_generated_data_backup()?;
        match self.import_public_document(profile_name, document_text, source_kind, source_path) {
            Ok(report) => {
                self.cleanup_generated_backups_keep_latest()?;
                Ok(report)
            }
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

    pub fn profile_record_keys(&self, profile_name: &str) -> Result<Vec<String>, GuiError> {
        let profile_name = validate_profile_name(profile_name)?;
        self.read_profile(&profile_name)?;
        Ok(self
            .read_records(&profile_name)?
            .iter()
            .map(record_semantic_key)
            .collect())
    }

    pub fn default_run_raw_path(&self) -> PathBuf {
        self.root
            .join("data/runs")
            .join(format!("raw-{}.jsonl", now_unique_stamp()))
    }
}
