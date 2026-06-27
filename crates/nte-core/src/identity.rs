use std::collections::BTreeMap;

use crate::{InternalRecord, public_record_id_from_material};

#[derive(Debug, Clone, Copy)]
pub struct RecordIdentityInput<'a> {
    pub record_type: &'a str,
    pub time: Option<&'a str>,
    pub pool_id: &'a str,
    pub item_id: &'a str,
    pub count: Option<i64>,
    pub roll_points: Option<i64>,
    pub roll_label_id: Option<&'a str>,
    pub secondary_item_id: Option<&'a str>,
    pub secondary_count: Option<i64>,
}

pub fn record_semantic_key(record: &InternalRecord) -> String {
    record_semantic_key_from_parts(RecordIdentityInput {
        record_type: &record.record_type,
        time: record.time.as_deref(),
        pool_id: &record.pool_id,
        item_id: &record.item_id,
        count: record.count,
        roll_points: record.roll_points,
        roll_label_id: record.roll_label_id.as_deref(),
        secondary_item_id: record.secondary_item_id.as_deref(),
        secondary_count: record.secondary_count,
    })
}

pub fn record_semantic_key_from_parts(input: RecordIdentityInput<'_>) -> String {
    public_record_id_from_material(&record_semantic_material(input))
}

pub fn stable_record_id_from_key(base_key: &str, occurrence: u64) -> String {
    if occurrence == 0 {
        return base_key.to_string();
    }
    public_record_id_from_material(&[
        base_key.to_string(),
        "duplicate_occurrence".to_string(),
        occurrence.to_string(),
    ])
}

pub fn assign_stable_record_ids(records: &mut [InternalRecord]) {
    let mut seen: BTreeMap<String, u64> = BTreeMap::new();
    for record in records {
        let key = record_semantic_key(record);
        let occurrence = seen.entry(key.clone()).or_default();
        record.record_id = stable_record_id_from_key(&key, *occurrence);
        *occurrence += 1;
    }
}

fn record_semantic_material(input: RecordIdentityInput<'_>) -> Vec<String> {
    vec![
        "semantic_v1".to_string(),
        input.record_type.to_string(),
        input.time.unwrap_or_default().to_string(),
        input.pool_id.to_string(),
        input.item_id.to_string(),
        input
            .count
            .map(|value| value.to_string())
            .unwrap_or_default(),
        input
            .roll_points
            .map(|value| value.to_string())
            .unwrap_or_default(),
        input.roll_label_id.unwrap_or_default().to_string(),
        input.secondary_item_id.unwrap_or_default().to_string(),
        input
            .secondary_count
            .map(|value| value.to_string())
            .unwrap_or_default(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(record_id: &str, time: &str, item_id: &str) -> InternalRecord {
        InternalRecord {
            record_id: record_id.to_string(),
            source_order: 0,
            record_type: "monopoly".to_string(),
            time: Some(time.to_string()),
            pool_id: "CardPool_NewRole".to_string(),
            item_id: item_id.to_string(),
            count: Some(1),
            roll_points: Some(6),
            roll_label_id: None,
            secondary_item_id: None,
            secondary_count: None,
        }
    }

    #[test]
    fn semantic_key_ignores_record_id_and_source_order() {
        let mut first = record("old", "2026-06-10 08:11:12", "fork_dustbin");
        let mut second = record("new", "2026-06-10 08:11:12", "fork_dustbin");
        first.source_order = 263;
        second.source_order = 643;

        assert_eq!(record_semantic_key(&first), record_semantic_key(&second));
    }

    #[test]
    fn stable_record_ids_keep_true_duplicate_occurrences_distinct() {
        let mut records = vec![
            record("a", "2026-06-10 08:11:12", "fork_dustbin"),
            record("b", "2026-06-10 08:11:12", "fork_dustbin"),
        ];

        assign_stable_record_ids(&mut records);

        assert_ne!(records[0].record_id, records[1].record_id);
        assert_eq!(records[0].record_id, record_semantic_key(&records[0]));
    }
}
