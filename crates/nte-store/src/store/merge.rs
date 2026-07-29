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
        let old_counts = semantic_counts(&old_records);
        let mut incoming_counts = BTreeMap::<String, u64>::new();
        let mut merged = old_records.clone();
        let mut inserted = 0_u64;
        let mut skipped = 0_u64;

        for mut record in incoming.drain(..) {
            let key = record_semantic_key(&record);
            let occurrence = incoming_counts.entry(key.clone()).or_default();
            let old_count = old_counts.get(&key).copied().unwrap_or_default();
            if *occurrence < old_count {
                skipped += 1;
            } else {
                record.record_id = stable_record_id_from_key(&key, *occurrence);
                inserted += 1;
                merged.push(record);
            }
            *occurrence += 1;
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
        self.ensure_profile_absent_except(name, None)
    }

    fn ensure_profile_absent_except(
        &self,
        name: &str,
        except: Option<&str>,
    ) -> Result<(), GuiError> {
        let key = profile_name_key(name);
        for profile in self.list_profiles()? {
            if except.is_some_and(|except| profile.name == except) {
                continue;
            }
            if profile_name_key(&profile.name) == key {
                return Err(ProfileError::AlreadyExists(name.to_string()).into());
            }
        }
        Ok(())
    }
}

fn semantic_counts(records: &[InternalRecord]) -> BTreeMap<String, u64> {
    let mut counts = BTreeMap::new();
    for record in records {
        *counts.entry(record_semantic_key(record)).or_default() += 1;
    }
    counts
}
