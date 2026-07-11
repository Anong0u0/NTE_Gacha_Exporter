use std::cmp::Ordering;

use crate::{DisplayRecord, InternalRecord};

pub fn compare_records_chronological(left: &InternalRecord, right: &InternalRecord) -> Ordering {
    compare_time_asc(left.time.as_deref(), right.time.as_deref())
        .then_with(|| left.source_order.cmp(&right.source_order))
        .then_with(|| left.record_id.cmp(&right.record_id))
}

pub fn compare_records_for_analysis(left: &InternalRecord, right: &InternalRecord) -> Ordering {
    compare_time_asc(left.time.as_deref(), right.time.as_deref())
        .then_with(|| left.source_order.cmp(&right.source_order))
        .then_with(|| left.record_id.cmp(&right.record_id))
}

pub fn compare_display_chronological(left: &DisplayRecord, right: &DisplayRecord) -> Ordering {
    compare_time_asc(left.time.as_deref(), right.time.as_deref())
        .then_with(|| left.source_order.cmp(&right.source_order))
        .then_with(|| left.record_id.cmp(&right.record_id))
}

pub fn compare_display_newest_first(left: &DisplayRecord, right: &DisplayRecord) -> Ordering {
    compare_time_desc(left.time.as_deref(), right.time.as_deref())
        .then_with(|| right.source_order.cmp(&left.source_order))
        .then_with(|| left.record_id.cmp(&right.record_id))
}

pub fn compare_time_asc(left: Option<&str>, right: Option<&str>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left.cmp(right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn compare_time_desc(left: Option<&str>, right: Option<&str>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => right.cmp(left),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::{DisplayRecord, GachaRuleView, InternalRecord, RecordDerived, ResolvedBanner};

    use super::{
        compare_display_chronological, compare_display_newest_first, compare_records_chronological,
        compare_records_for_analysis,
    };

    fn internal(record_id: &str, time: &str, source_order: u64) -> InternalRecord {
        InternalRecord {
            record_id: record_id.to_string(),
            source_order,
            record_type: "monopoly".to_string(),
            time: Some(time.to_string()),
            pool_id: "CardPool_Character".to_string(),
            item_id: "1004".to_string(),
            count: Some(1),
            roll_points: Some(1),
            roll_label_id: None,
            secondary_item_id: None,
            secondary_count: None,
        }
    }

    fn display(record_id: &str, time: &str, source_order: u64) -> DisplayRecord {
        DisplayRecord {
            record_id: record_id.to_string(),
            source_order,
            record_type: "monopoly".to_string(),
            time: Some(time.to_string()),
            pool_id: "CardPool_Character".to_string(),
            pool_kind: crate::PoolKind::MonopolyLimited,
            pool_label: "pool".to_string(),
            banner: resolved_banner(),
            item_id: "1004".to_string(),
            item_name: "item".to_string(),
            item_asset_refs: BTreeMap::new(),
            item_kind: crate::ItemKind::Character,
            rarity: Some(5),
            count: Some(1),
            roll_points: Some(1),
            roll_label_id: None,
            roll_label: None,
            roll_bucket: crate::RollBucket::One,
            fork_result_mark: None,
            secondary_item_name: None,
            secondary_item_id: None,
            secondary_item_asset_refs: BTreeMap::new(),
            secondary_count: None,
            derived: derived(record_id),
        }
    }

    fn resolved_banner() -> ResolvedBanner {
        ResolvedBanner {
            resolution_issue: None,
            reason: None,
            banner_id: None,
            pool_id: Some("CardPool_Character".to_string()),
            pool_kind: Some("monopoly_limited".to_string()),
            banner_type: None,
            title: None,
            version: None,
            start_at: None,
            end_at: None,
            timezone: None,
            rate_up_5: Vec::new(),
            rate_up_4: Vec::new(),
            rule_id: None,
            asset_refs: BTreeMap::new(),
        }
    }

    fn derived(record_id: &str) -> RecordDerived {
        RecordDerived {
            record_id: record_id.to_string(),
            banner_id: None,
            banner_version: None,
            counts_as_pull: true,
            global_pull_no: None,
            pull_no_in_pool_kind: None,
            pull_no_in_banner: None,
            pity_5_before: 0,
            pity_5_after: 1,
            ten_pull_progress_before: None,
            ten_pull_progress_after: None,
            hit_rarity: Some(5),
            rate_up_result: crate::RateUpResult::Unknown,
            pity_badge: None,
            guarantee_5_before: None,
            guarantee_5_after: None,
            fork_up_pity_before: None,
            fork_up_pity_after: None,
            fork_forced_up: None,
            rule: GachaRuleView {
                resolution_issue: None,
                reason: None,
                rule_id: None,
                pool_kind: crate::PoolKind::MonopolyLimited,
                hard_pity_5: None,
                hard_up_pity_5: None,
                pickup_win_rate_5: None,
                has_guarantee_5: None,
                guarantee_scope: None,
                carry_scope: None,
            },
        }
    }

    #[test]
    fn same_timestamp_source_order_is_chronological_low_to_high() {
        let mut records = [
            internal("newer", "2026-01-01 00:00:00", 2),
            internal("older", "2026-01-01 00:00:00", 1),
        ];
        records.sort_by(compare_records_chronological);
        assert_eq!(
            records
                .iter()
                .map(|record| record.record_id.as_str())
                .collect::<Vec<_>>(),
            vec!["older", "newer"]
        );

        records.sort_by(compare_records_for_analysis);
        assert_eq!(
            records
                .iter()
                .map(|record| record.record_id.as_str())
                .collect::<Vec<_>>(),
            vec!["older", "newer"]
        );
    }

    #[test]
    fn same_timestamp_newest_first_uses_source_order_high_to_low() {
        let mut records = [
            display("older", "2026-01-01 00:00:00", 1),
            display("newer", "2026-01-01 00:00:00", 2),
        ];

        records.sort_by(compare_display_newest_first);
        assert_eq!(
            records
                .iter()
                .map(|record| record.record_id.as_str())
                .collect::<Vec<_>>(),
            vec!["newer", "older"]
        );

        records.sort_by(compare_display_chronological);
        assert_eq!(
            records
                .iter()
                .map(|record| record.record_id.as_str())
                .collect::<Vec<_>>(),
            vec!["older", "newer"]
        );
    }
}
