use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::capture_protocol::{ParsedRow, row_public_time};
use nte_core::GuiError;
use nte_core::{MapData, load_map};

#[derive(Debug, Clone)]
pub struct CapturePublicRecord {
    pub record_id: String,
    pub record_type: String,
    pub pool_id: Option<String>,
    pub value: Value,
}

pub struct CaptureRecordBuilder {
    map: MapData,
    seen_record_ids: BTreeMap<String, u64>,
}

struct CanonicalRow<'a> {
    row: &'a ParsedRow,
    item_id: String,
    secondary_item_id: Option<String>,
}

impl CaptureRecordBuilder {
    pub fn new(locale: &str) -> Result<Self, GuiError> {
        Ok(Self {
            map: load_map(locale)?,
            seen_record_ids: BTreeMap::new(),
        })
    }

    pub fn build_records(&mut self, rows: &[ParsedRow]) -> Vec<CapturePublicRecord> {
        rows.iter().map(|row| self.build_record(row)).collect()
    }

    pub fn build_record(&mut self, row: &ParsedRow) -> CapturePublicRecord {
        let canonical = canonical_row(row, &self.map);
        let record_id = self.next_record_id(&canonical);
        let value = public_record(&canonical, record_id.clone());
        CapturePublicRecord {
            record_id,
            record_type: row.record_type.as_str().to_string(),
            pool_id: row.pool_id.clone(),
            value,
        }
    }

    fn next_record_id(&mut self, row: &CanonicalRow<'_>) -> String {
        let base_id = record_id_from_material(&record_id_material(row));
        let occurrence = self.seen_record_ids.entry(base_id.clone()).or_default();
        let record_id = if *occurrence == 0 {
            base_id
        } else {
            let mut material = record_id_material(row);
            material.push("duplicate_occurrence".to_string());
            material.push(occurrence.to_string());
            record_id_from_material(&material)
        };
        *occurrence += 1;
        record_id
    }
}

pub fn build_capture_document(rows: &[ParsedRow], locale: &str) -> Result<Value, GuiError> {
    let map = load_map(locale)?;
    let canonical_rows = rows
        .iter()
        .map(|row| canonical_row(row, &map))
        .collect::<Vec<_>>();
    let record_ids = record_ids(&canonical_rows);
    let records = canonical_rows
        .iter()
        .zip(record_ids)
        .map(|(row, record_id)| public_record(row, record_id))
        .collect::<Vec<_>>();
    Ok(json!({
        "info": {
            "schema": "nte-gacha-exporter-export",
            "schema_version": "2.0",
            "export_app": "nte-gacha-exporter-desktop",
            "export_app_version": env!("CARGO_PKG_VERSION"),
            "export_timestamp": now_stamp(),
            "locale": locale,
            "name_source": "localization_map",
            "time_source": "decoded_dotnet_ticks",
            "privacy": "sanitized"
        },
        "nte": {
            "list": records
        }
    }))
}

fn canonical_row<'a>(row: &'a ParsedRow, map: &MapData) -> CanonicalRow<'a> {
    CanonicalRow {
        row,
        item_id: map.canonical_item_id(&row.item_id).to_string(),
        secondary_item_id: row
            .secondary_item_id
            .as_deref()
            .map(|value| map.canonical_item_id(value).to_string()),
    }
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
    fn sample_fixture_matches_record_ids() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .unwrap();
        let raw_path = root.join("fixtures/sample.raw.jsonl");
        let rows = capture_raw::read_raw_capture(&raw_path).unwrap();
        let document = build_capture_document(&rows.rows, "zh-Hant").unwrap();
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

        let mut builder = CaptureRecordBuilder::new("zh-Hant").unwrap();
        let incremental_records = builder
            .build_records(&rows.rows)
            .into_iter()
            .map(|record| record.value)
            .collect::<Vec<_>>();
        assert_eq!(incremental_records.as_slice(), records.as_slice());
    }
}
