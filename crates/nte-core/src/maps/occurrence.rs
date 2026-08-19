use crate::model::{
    BannerOccurrenceResolution, InternalRecord, ResolvedBannerOccurrence,
};

const OBSERVED_OCCURRENCE_GAP_DAYS: i64 = 42;

impl MapData {
    pub fn resolve_banner_occurrences(
        &self,
        records: &[InternalRecord],
    ) -> BTreeMap<String, ResolvedBannerOccurrence> {
        let mut resolved = BTreeMap::new();
        let mut unmatched: BTreeMap<String, Vec<(&InternalRecord, chrono::NaiveDateTime, String)>> =
            BTreeMap::new();

        for record in records {
            let banner_source = self.resolve_banner_source(BannerResolveInput {
                pool_id: &record.pool_id,
                lottery_id: record.lottery_id.as_deref(),
            });
            let Some(source_id) = source_id_for_record(record) else {
                resolved.insert(
                    record.record_id.clone(),
                    unresolved_occurrence(None, "record has no banner source ID"),
                );
                continue;
            };
            if banner_source.resolution_issue == Some(BannerResolutionIssue::LotteryPoolMismatch) {
                let detail = banner_source
                    .reason
                    .as_deref()
                    .unwrap_or("lottery ID does not belong to record pool");
                resolved.insert(
                    record.record_id.clone(),
                    unresolved_occurrence(
                        Some(source_id.clone()),
                        format!(
                            "banner source/pool conflict: source_id={source_id} pool_id={}; {detail}",
                            record.pool_id
                        ),
                    ),
                );
                continue;
            }
            let Some(normalized_time) = normalize_game_time(record.time.as_deref()) else {
                resolved.insert(
                    record.record_id.clone(),
                    unresolved_occurrence(Some(source_id), "record has no valid time"),
                );
                continue;
            };
            let parsed_time = chrono::NaiveDateTime::parse_from_str(
                &normalized_time,
                "%Y-%m-%d %H:%M:%S",
            )
            .expect("normalized game time must parse");
            if let Some(occurrence) = self
                .banner_occurrences
                .iter()
                .find(|occurrence| {
                    occurrence.source_id == source_id
                        && occurrence_matches_time(occurrence, &normalized_time)
                })
            {
                resolved.insert(
                    record.record_id.clone(),
                    catalog_occurrence(occurrence),
                );
            } else {
                unmatched.entry(source_id).or_default().push((
                    record,
                    parsed_time,
                    normalized_time,
                ));
            }
        }

        for (source_id, mut source_records) in unmatched {
            source_records.sort_by(|left, right| {
                left.1
                    .cmp(&right.1)
                    .then_with(|| left.0.source_order.cmp(&right.0.source_order))
                    .then_with(|| left.0.record_id.cmp(&right.0.record_id))
            });
            let mut segment_start = 0;
            for index in 1..=source_records.len() {
                let split = index == source_records.len()
                    || source_records[index].1 - source_records[index - 1].1
                        > chrono::Duration::days(OBSERVED_OCCURRENCE_GAP_DAYS);
                if !split {
                    continue;
                }
                let segment = &source_records[segment_start..index];
                let first_time = &segment[0].2;
                let occurrence_id = observed_occurrence_id(&source_id, first_time);
                for (record, _, _) in segment {
                    resolved.insert(
                        record.record_id.clone(),
                        ResolvedBannerOccurrence {
                            occurrence_id: Some(occurrence_id.clone()),
                            source_id: Some(source_id.clone()),
                            resolution_source: BannerOccurrenceResolution::ObservedGap,
                            reason: None,
                            version: None,
                            active_from: Some(first_time.clone()),
                            active_until: None,
                        },
                    );
                }
                segment_start = index;
            }
        }
        resolved
    }
}

fn source_id_for_record(record: &InternalRecord) -> Option<String> {
    match record.pool_id.as_str() {
        "CardPool_Character" | "CardPool_NewRole" => record
            .lottery_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        pool_id if pool_id.starts_with("ForkLottery_") => Some(pool_id.to_string()),
        _ => None,
    }
}

fn catalog_occurrence(occurrence: &MapBannerOccurrence) -> ResolvedBannerOccurrence {
    ResolvedBannerOccurrence {
        occurrence_id: Some(occurrence.occurrence_id.clone()),
        source_id: Some(occurrence.source_id.clone()),
        resolution_source: BannerOccurrenceResolution::Catalog,
        reason: None,
        version: occurrence.version.clone(),
        active_from: occurrence.active_from.clone(),
        active_until: occurrence.active_until.clone(),
    }
}

fn unresolved_occurrence(
    source_id: Option<String>,
    reason: impl Into<String>,
) -> ResolvedBannerOccurrence {
    ResolvedBannerOccurrence {
        occurrence_id: None,
        source_id,
        resolution_source: BannerOccurrenceResolution::Unresolved,
        reason: Some(reason.into()),
        version: None,
        active_from: None,
        active_until: None,
    }
}

fn observed_occurrence_id(source_id: &str, first_time: &str) -> String {
    let compact = first_time
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>();
    format!("{source_id}@observed-{compact}")
}

#[cfg(test)]
mod occurrence_tests {
    use super::*;
    use crate::{derive_records, load_map, MapBannerOccurrence};

    fn record(
        record_id: &str,
        pool_id: &str,
        lottery_id: Option<&str>,
        time: Option<&str>,
    ) -> InternalRecord {
        InternalRecord {
            record_id: record_id.to_string(),
            source_order: 0,
            record_type: if pool_id.starts_with("ForkLottery_") {
                "fork".to_string()
            } else {
                "monopoly".to_string()
            },
            time: time.map(ToOwned::to_owned),
            pool_id: pool_id.to_string(),
            lottery_id: lottery_id.map(ToOwned::to_owned),
            item_id: "1003".to_string(),
            count: Some(1),
            roll_points: Some(1),
            roll_label_id: None,
            secondary_item_id: None,
            secondary_count: None,
        }
    }

    #[test]
    fn catalog_distinguishes_nanali_occurrences_and_covers_global_window() {
        let map = load_map("zh-Hant").expect("map should load");
        let records = vec![
            record(
                "first",
                "CardPool_Character",
                Some("Lottery_Nanali"),
                Some("2026-04-30 06:00:00"),
            ),
            record(
                "rerun-mainland",
                "CardPool_Character",
                Some("Lottery_Nanali"),
                Some("2026-08-13 06:00:00"),
            ),
            record(
                "rerun-overseas",
                "CardPool_Character",
                Some("Lottery_Nanali"),
                Some("2026-08-20 06:00:00"),
            ),
        ];

        let resolved = map.resolve_banner_occurrences(&records);

        assert_eq!(
            resolved["first"].occurrence_id.as_deref(),
            Some("Lottery_Nanali@2026-04")
        );
        for record_id in ["rerun-mainland", "rerun-overseas"] {
            assert_eq!(
                resolved[record_id].occurrence_id.as_deref(),
                Some("Lottery_Nanali@2026-08")
            );
            assert_eq!(
                resolved[record_id].resolution_source,
                BannerOccurrenceResolution::Catalog
            );
        }
    }

    #[test]
    fn observed_gap_splits_only_when_strictly_greater_than_42_days() {
        let map = load_map("zh-Hant").expect("map should load");
        let records = vec![
            record(
                "first",
                "CardPool_Character",
                Some("Lottery_Future"),
                Some("2027-01-01 00:00:00"),
            ),
            record(
                "exact-boundary",
                "CardPool_Character",
                Some("Lottery_Future"),
                Some("2027-02-12 00:00:00"),
            ),
            record(
                "after-gap",
                "CardPool_Character",
                Some("Lottery_Future"),
                Some("2027-03-27 00:00:00"),
            ),
        ];

        let resolved = map.resolve_banner_occurrences(&records);

        assert_eq!(
            resolved["first"].occurrence_id,
            resolved["exact-boundary"].occurrence_id
        );
        assert_ne!(
            resolved["exact-boundary"].occurrence_id,
            resolved["after-gap"].occurrence_id
        );
        assert!(resolved.values().all(|occurrence| {
            occurrence.resolution_source == BannerOccurrenceResolution::ObservedGap
        }));
    }

    #[test]
    fn unknown_monopoly_and_fork_sources_form_observed_occurrences() {
        let map = load_map("zh-Hant").expect("map should load");
        let records = vec![
            record(
                "monopoly",
                "CardPool_Character",
                Some("Lottery_Future"),
                Some("2027-01-01 00:00:00"),
            ),
            record(
                "fork",
                "ForkLottery_Future",
                None,
                Some("2027-01-01 00:00:00"),
            ),
        ];

        let resolved = map.resolve_banner_occurrences(&records);

        assert_eq!(
            resolved["monopoly"].source_id.as_deref(),
            Some("Lottery_Future")
        );
        assert_eq!(
            resolved["fork"].source_id.as_deref(),
            Some("ForkLottery_Future")
        );
        assert!(resolved.values().all(|occurrence| {
            occurrence.resolution_source == BannerOccurrenceResolution::ObservedGap
                && occurrence.occurrence_id.is_some()
        }));
    }

    #[test]
    fn missing_source_or_invalid_time_stays_unresolved() {
        let map = load_map("zh-Hant").expect("map should load");
        let records = vec![
            record(
                "missing-source",
                "CardPool_Character",
                None,
                Some("2027-01-01 00:00:00"),
            ),
            record(
                "missing-time",
                "CardPool_Character",
                Some("Lottery_Future"),
                None,
            ),
            record(
                "invalid-time",
                "ForkLottery_Future",
                None,
                Some("invalid"),
            ),
        ];

        let resolved = map.resolve_banner_occurrences(&records);

        assert!(resolved.values().all(|occurrence| {
            occurrence.resolution_source == BannerOccurrenceResolution::Unresolved
                && occurrence.occurrence_id.is_none()
        }));
    }

    #[test]
    fn pool_mismatches_stay_unresolved_and_preserve_wire_source_id() {
        let map = load_map("zh-Hant").expect("map should load");
        let records = vec![
            record(
                "permanent",
                "CardPool_NewRole",
                Some("Lottery_Permanent"),
                Some("2026-08-13 06:00:00"),
            ),
            record(
                "limited-with-permanent-id",
                "CardPool_Character",
                Some("Lottery_Permanent"),
                Some("2026-08-13 06:00:00"),
            ),
            record(
                "standard-with-limited-id",
                "CardPool_NewRole",
                Some("Lottery_Nanali"),
                Some("2026-08-13 06:00:00"),
            ),
        ];

        let resolved = map.resolve_banner_occurrences(&records);

        assert_eq!(
            resolved["permanent"].occurrence_id.as_deref(),
            Some("Lottery_Permanent@permanent")
        );
        for (record_id, source_id) in [
            ("limited-with-permanent-id", "Lottery_Permanent"),
            ("standard-with-limited-id", "Lottery_Nanali"),
        ] {
            let occurrence = &resolved[record_id];
            assert_eq!(occurrence.source_id.as_deref(), Some(source_id));
            assert_eq!(
                occurrence.resolution_source,
                BannerOccurrenceResolution::Unresolved
            );
            assert_eq!(occurrence.occurrence_id, None);
            assert!(
                occurrence
                    .reason
                    .as_deref()
                    .is_some_and(|reason| reason.contains("source/pool conflict"))
            );
        }
    }

    #[test]
    fn catalog_update_replaces_observed_occurrence_without_changing_record_id() {
        let map = load_map("zh-Hant").expect("map should load");
        let record = record(
            "stable-record",
            "CardPool_Character",
            Some("Lottery_Nanali"),
            Some("2027-01-10 00:00:00"),
        );
        let before = derive_records(std::slice::from_ref(&record), &map)
            .expect("record should derive before catalog update");
        assert!(before[0]
            .banner_occurrence_id
            .as_deref()
            .is_some_and(|id| id.contains("@observed-")));

        let mut updated = map;
        updated.banner_occurrences.push(MapBannerOccurrence {
            occurrence_id: "Lottery_Nanali@2027-01".to_string(),
            source_id: "Lottery_Nanali".to_string(),
            active_from: Some("2027-01-01 00:00:00".to_string()),
            active_until: Some("2027-02-01 00:00:00".to_string()),
            version: Some("future".to_string()),
        });
        let after = derive_records(&[record], &updated)
            .expect("record should derive after catalog update");

        assert_eq!(before[0].record_id, after[0].record_id);
        assert_eq!(
            after[0].banner_occurrence_id.as_deref(),
            Some("Lottery_Nanali@2027-01")
        );
    }
}
