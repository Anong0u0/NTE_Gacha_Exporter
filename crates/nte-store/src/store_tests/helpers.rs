use std::io::Write;

use nte_core::{RecordIdentityInput, record_semantic_key_from_parts, stable_record_id_from_key};
use serde_json::json;

pub(super) fn public_document(mut records: Vec<serde_json::Value>) -> String {
    for (index, record) in records.iter_mut().enumerate() {
        if let Some(object) = record.as_object_mut() {
            object
                .entry("source_order".to_string())
                .or_insert_with(|| json!(index));
        }
    }
    json!({
        "info": {
            "schema": nte_core::PUBLIC_JSON_SCHEMA,
            "schema_version": "2.0"
        },
        "nte": {
            "list": records
        }
    })
    .to_string()
}

pub(super) fn record(
    record_id: &str,
    pool_id: &str,
    item_id: &str,
    time: &str,
) -> serde_json::Value {
    json!({
        "record_id": record_id,
        "record_type": if pool_id.starts_with("ForkLottery_") { "fork" } else { "monopoly" },
        "time": time,
        "pool_id": pool_id,
        "pool_name": "display must be ignored",
        "item_id": item_id,
        "item_name": "display must be ignored",
        "count": 1,
        "roll_points": 1,
        "roll_label": "display must be ignored"
    })
}

pub(super) fn record_with_options(
    record_id: &str,
    pool_id: &str,
    item_id: &str,
    time: Option<&str>,
    roll_points: Option<i64>,
) -> serde_json::Value {
    json!({
        "record_id": record_id,
        "record_type": if pool_id.starts_with("ForkLottery_") { "fork" } else { "monopoly" },
        "time": time,
        "pool_id": pool_id,
        "pool_name": "display must be ignored",
        "item_id": item_id,
        "item_name": "display must be ignored",
        "count": 1,
        "roll_points": roll_points,
        "roll_label": "display must be ignored"
    })
}

pub(super) fn expected_record_id(record: &serde_json::Value) -> String {
    expected_record_id_with_occurrence(record, 0)
}

pub(super) fn expected_record_ids(records: &[serde_json::Value]) -> Vec<String> {
    records.iter().map(expected_record_id).collect()
}

pub(super) fn expected_record_id_with_occurrence(
    record: &serde_json::Value,
    occurrence: u64,
) -> String {
    let pool_id = record["pool_id"].as_str().unwrap_or_default();
    let roll_points = optional_roll_points(record["roll_points"].as_i64());
    let roll_label_id = record["roll_label_id"]
        .as_str()
        .map(str::to_string)
        .or_else(|| {
            record["roll_points"]
                .as_i64()
                .and_then(roll_label_id_from_sentinel)
        });
    let key = record_semantic_key_from_parts(RecordIdentityInput {
        record_type: record["record_type"].as_str().unwrap_or_else(|| {
            if pool_id.starts_with("ForkLottery_") {
                "fork"
            } else {
                "monopoly"
            }
        }),
        time: record["time"].as_str(),
        pool_id,
        item_id: record["item_id"].as_str().unwrap_or_default(),
        count: record["count"].as_i64(),
        roll_points,
        roll_label_id: roll_label_id.as_deref(),
        secondary_item_id: record["secondary_item_id"].as_str(),
        secondary_count: record["secondary_count"].as_i64(),
    });
    stable_record_id_from_key(&key, occurrence)
}

pub(super) fn expected_record_id_for(pool_id: &str, item_id: &str, time: &str) -> String {
    expected_record_id(&record("expected", pool_id, item_id, time))
}

fn optional_roll_points(value: Option<i64>) -> Option<i64> {
    value.filter(|value| !matches!(value, 0 | 4_294_967_295))
}

fn roll_label_id_from_sentinel(value: i64) -> Option<String> {
    match value {
        0 => Some("BPUI_LotteryResult_jidianzengli".to_string()),
        4_294_967_295 => Some("BPUI_LotteryResult_chenmiandi".to_string()),
        _ => None,
    }
}

pub(super) fn write_backup_zip(path: &std::path::Path, files: &[(&str, String)]) {
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::FileOptions::default();
    let names = files
        .iter()
        .map(|(name, _)| (*name).to_string())
        .collect::<Vec<_>>();
    for (name, text) in files {
        zip.start_file(*name, options).unwrap();
        zip.write_all(text.as_bytes()).unwrap();
    }
    zip.start_file("manifest.json", options).unwrap();
    zip.write_all(
        serde_json::to_string(&json!({
            "schema": "nte-gacha-exporter-data-backup",
            "schema_version": 1,
            "created_at": "1",
            "files": names,
        }))
        .unwrap()
        .as_bytes(),
    )
    .unwrap();
    zip.finish().unwrap();
}
