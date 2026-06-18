use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::capture_protocol::{row_public_time, ParseWarning, ParsedRow};
use crate::maps::load_map;
use crate::model::GuiError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawReplayResult {
    pub document: Value,
    pub records_count: u64,
}

struct CanonicalRow<'a> {
    row: &'a ParsedRow,
    item_id: String,
    secondary_item_id: Option<String>,
}

pub fn build_capture_document(
    rows: &[ParsedRow],
    warnings: &[ParseWarning],
    locale: &str,
    source: &str,
) -> Result<Value, GuiError> {
    let map = load_map(locale)?;
    let canonical_rows = rows
        .iter()
        .map(|row| CanonicalRow {
            row,
            item_id: map.canonical_item_id(&row.item_id).to_string(),
            secondary_item_id: row
                .secondary_item_id
                .as_deref()
                .map(|value| map.canonical_item_id(value).to_string()),
        })
        .collect::<Vec<_>>();
    let record_ids = record_ids(&canonical_rows);
    let records = canonical_rows
        .iter()
        .zip(record_ids)
        .map(|(row, record_id)| public_record(row, record_id))
        .collect::<Vec<_>>();
    let mut by_record_type: BTreeMap<String, u64> = BTreeMap::new();
    let mut by_pool: BTreeMap<String, u64> = BTreeMap::new();
    let mut times = Vec::new();
    for record in &records {
        if let Some(record_type) = record.get("record_type").and_then(Value::as_str) {
            *by_record_type.entry(record_type.to_string()).or_default() += 1;
        }
        if let Some(pool_id) = record.get("pool_id").and_then(Value::as_str) {
            *by_pool.entry(pool_id.to_string()).or_default() += 1;
        }
        if let Some(time) = record.get("time").and_then(Value::as_str) {
            times.push(time.to_string());
        }
    }
    times.sort();
    let time_range = if times.is_empty() {
        Value::Null
    } else {
        json!([times[0], times[times.len() - 1]])
    };
    Ok(json!({
        "info": {
            "schema": "nte-gacha-export",
            "schema_version": "2.0",
            "export_app": "nte-gacha-desktop",
            "export_app_version": env!("CARGO_PKG_VERSION"),
            "export_timestamp": now_stamp(),
            "locale": locale,
            "name_source": "localization_map",
            "time_source": "decoded_dotnet_ticks",
            "privacy": "sanitized"
        },
        "nte": {
            "list": records
        },
        "_debug": {
            "source": source,
            "summary": {
                "record_count": records.len(),
                "time_range": time_range,
                "by_record_type": by_record_type,
                "by_pool": by_pool,
                "warning_count": warnings.len()
            },
            "warnings": warnings,
            "raw_rows": rows
        }
    }))
}

fn public_record(row: &CanonicalRow<'_>, record_id: String) -> Value {
    let mut object = Map::new();
    insert_string(&mut object, "record_id", record_id);
    insert_string(&mut object, "record_type", row.row.record_type.as_str());
    insert_opt_string(&mut object, "time", row_public_time(row.row));
    insert_opt_string(&mut object, "pool_id", row.row.pool_id.clone());
    insert_string(&mut object, "item_id", row.item_id.clone());
    object.insert("count".to_string(), json!(row.row.count));
    if let Some(roll_points) = row.row.roll_points {
        object.insert("roll_points".to_string(), json!(roll_points));
    }
    if let Some(secondary_item_id) = secondary_item_id(row) {
        insert_string(&mut object, "secondary_item_id", secondary_item_id);
        if let Some(secondary_count) = row.row.secondary_count {
            object.insert("secondary_count".to_string(), json!(secondary_count));
        }
    }
    Value::Object(object)
}

fn secondary_item_id(row: &CanonicalRow<'_>) -> Option<String> {
    let secondary = row.secondary_item_id.as_ref()?;
    (secondary != &row.item_id).then(|| secondary.clone())
}

fn record_ids(rows: &[CanonicalRow<'_>]) -> Vec<String> {
    let base_ids = rows
        .iter()
        .map(|row| record_id_from_material(&record_id_material(row)))
        .collect::<Vec<_>>();
    let mut totals: BTreeMap<String, u64> = BTreeMap::new();
    for id in &base_ids {
        *totals.entry(id.clone()).or_default() += 1;
    }
    let mut seen: BTreeMap<String, u64> = BTreeMap::new();
    let mut ids = Vec::with_capacity(rows.len());
    for (row, base_id) in rows.iter().zip(base_ids) {
        if totals.get(&base_id) == Some(&1) {
            ids.push(base_id);
            continue;
        }
        let occurrence = seen.entry(base_id.clone()).or_default();
        if *occurrence == 0 {
            ids.push(base_id);
        } else {
            let mut material = record_id_material(row);
            material.push("duplicate_occurrence".to_string());
            material.push(occurrence.to_string());
            ids.push(record_id_from_material(&material));
        }
        *occurrence += 1;
    }
    ids
}

fn record_id_material(row: &CanonicalRow<'_>) -> Vec<String> {
    vec![
        row.row.record_type.as_str().to_string(),
        row.row.ticks.to_string(),
        row.row.pool_id.clone().unwrap_or_default(),
        row.row.source.row_index.to_string(),
        row.row
            .roll_points
            .map(|value| value.to_string())
            .unwrap_or_default(),
        row.item_id.clone(),
        row.row.count.to_string(),
        row.secondary_item_id.clone().unwrap_or_default(),
        row.row
            .secondary_count
            .map(|value| value.to_string())
            .unwrap_or_default(),
    ]
}

fn record_id_from_material(material: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(material.join("\x1f").as_bytes());
    hex::encode(hasher.finalize())
}

fn insert_string(object: &mut Map<String, Value>, key: &str, value: impl Into<String>) {
    let value = value.into();
    if !value.is_empty() {
        object.insert(key.to_string(), Value::String(value));
    }
}

fn insert_opt_string(object: &mut Map<String, Value>, key: &str, value: Option<String>) {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        object.insert(key.to_string(), Value::String(value));
    }
}

fn now_stamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::capture_raw;

    use super::*;

    #[test]
    fn sample_fixture_matches_python_record_ids() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .unwrap();
        let raw_path = root.join("tests/fixtures/sample.raw.jsonl");
        let rows = capture_raw::read_raw_capture(&raw_path).unwrap();
        let document =
            build_capture_document(&rows.rows, &rows.warnings, "zh-Hant", "test").unwrap();
        let records = document["nte"]["list"].as_array().unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(
            records[0]["record_id"],
            "02539eac1cdcfe813b158e11d27f81742e76fd30c05b93cf42615b5dd43f9c1f"
        );
        assert_eq!(
            records[1]["record_id"],
            "c56d8202675fe561fbfaca53f78f01eaa5184c66e48b20573841f883e8c84fbc"
        );
    }
}
