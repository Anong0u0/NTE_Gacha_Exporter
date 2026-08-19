impl JsonStore {
fn merge_records(
        &self,
        profile_name: &str,
        mut incoming: Vec<InternalRecord>,
        source_kind: &str,
        source_path: Option<&str>,
        map: &MapData,
    ) -> Result<ImportReport, GuiError> {
        let mut old_records = self.read_records(profile_name)?;
        canonicalize_records_against_map(&mut old_records, map);
        let old_last_run = self.read_last_run(profile_name)?;
        let mut merged = old_records.clone();
        let (inserted, skipped) = merge_record_sets(&mut merged, incoming.drain(..))?;
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
}
