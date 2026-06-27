impl JsonStore {
    pub fn profile_analysis_view(
        &self,
        profile_name: &str,
        locale: &str,
        selection: &DashboardSelection,
        record_filter: &RecordFilter,
    ) -> Result<ProfileAnalysisView, GuiError> {
        let profile = self.profile_for_api(profile_name)?;
        let map = load_map(locale)?;
        let records = self.read_records(&profile.name)?;
        let last_run = self.read_last_run(&profile.name)?;
        profile_analysis_view(profile, last_run, &records, &map, selection, record_filter)
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

    pub fn dashboard_selection_detail(
        &self,
        profile_name: &str,
        locale: &str,
        selection: &DashboardSelection,
    ) -> Result<DashboardSelectionDetail, GuiError> {
        let profile = self.profile_for_api(profile_name)?;
        let map = load_map(locale)?;
        let records = self.read_records(&profile.name)?;
        dashboard_selection_detail(&records, &map, selection)
    }

    pub fn dashboard_scope_detail(
        &self,
        profile_name: &str,
        locale: &str,
        selection: &DashboardSelection,
    ) -> Result<DashboardSelectionDetail, GuiError> {
        self.dashboard_selection_detail(profile_name, locale, selection)
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

    pub fn record_page(
        &self,
        profile_name: &str,
        locale: &str,
        filter: &RecordFilter,
    ) -> Result<RecordList, GuiError> {
        self.list_records(profile_name, locale, filter)
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

    fn bootstrap(&self, defaults: &StoreDefaults) -> Result<(), GuiError> {
        fs::create_dir_all(self.root.join("data"))?;
        fs::create_dir_all(self.profiles_dir())?;
        fs::create_dir_all(self.root.join("data/backups"))?;
        fs::create_dir_all(self.root.join("data/runs"))?;
        if !self.settings_path().exists() {
            validate_locale(&defaults.locale)?;
            validate_ui_locale(&defaults.ui_locale)?;
            self.write_settings(&DiskSettings {
                schema_version: 1,
                active_profile: DEFAULT_PROFILE.to_string(),
                locale: defaults.locale.clone(),
                ui_locale: defaults.ui_locale.clone(),
                update_channel: DEFAULT_UPDATE_CHANNEL.to_string(),
                check_updates_on_startup: DEFAULT_CHECK_UPDATES_ON_STARTUP,
                skipped_update_version: None,
                capture_auto_page_enabled: true,
                capture_full_update_enabled: false,
            })?;
        } else {
            let mut settings = self.read_settings()?;
            if settings.locale.trim().is_empty() {
                settings.locale = defaults.locale.clone();
            }
            settings.ui_locale =
                normalize_ui_locale_or_default(&settings.ui_locale, &defaults.ui_locale)?;
            validate_locale(&settings.locale)?;
            self.write_settings(&settings)?;
        }
        if !self.profile_dir(DEFAULT_PROFILE).exists() {
            self.create_profile(DEFAULT_PROFILE)?;
        }
        Ok(())
    }
}
