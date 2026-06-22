use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::protocol::{ParsedRow, RecordType, row_public_time};
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
    next_source_order: u64,
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
            next_source_order: 0,
        })
    }

    pub fn build_records(&mut self, rows: &[ParsedRow]) -> Vec<CapturePublicRecord> {
        rows.iter().map(|row| self.build_record(row)).collect()
    }

    pub fn build_record(&mut self, row: &ParsedRow) -> CapturePublicRecord {
        let canonical = canonical_row(row, &self.map);
        let record_id = self.next_record_id(&canonical);
        let source_order = self.next_source_order;
        self.next_source_order += 1;
        let value = public_record(&canonical, record_id.clone(), source_order);
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
        .enumerate()
        .map(|(source_order, (row, record_id))| public_record(row, record_id, source_order as u64))
        .collect::<Vec<_>>();
    Ok(json!({
        "info": {
            "schema": "nte-gacha-exporter-export",
            "schema_version": "3.0",
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

fn public_record(row: &CanonicalRow<'_>, record_id: String, source_order: u64) -> Value {
    let mut object = Map::new();
    insert_string(&mut object, "record_id", record_id);
    object.insert("source_order".to_string(), json!(source_order));
    insert_string(&mut object, "record_type", row.row.record_type.as_str());
    insert_opt_string(&mut object, "time", row_public_time(row.row));
    insert_opt_string(&mut object, "pool_id", row.row.pool_id.clone());
    insert_string(&mut object, "item_id", row.item_id.clone());
    object.insert("count".to_string(), json!(row.row.count));
    if let Some(roll_points) = row.row.roll_points {
        object.insert("roll_points".to_string(), json!(roll_points));
    }
    insert_opt_string(&mut object, "roll_label_id", row.row.roll_label_id.clone());
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
    if row.row.record_type == RecordType::Fork {
        if let Some(query_high) = row.row.source.query_high {
            let group_draw_index = row.row.source.row_index + if query_high { 0 } else { 5 };
            return vec![
                row.row.record_type.as_str().to_string(),
                "protocol_v2".to_string(),
                row.row.ticks.to_string(),
                row.row.pool_id.clone().unwrap_or_default(),
                group_draw_index.to_string(),
                row.item_id.clone(),
                row.row.count.to_string(),
            ];
        }
    }

    let mut material = vec![
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
    ];
    if let Some(roll_label_id) = row.row.roll_label_id.as_ref() {
        material.push("roll_label_id".to_string());
        material.push(roll_label_id.clone());
    }
    material
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

    use crate::protocol::SourceRef;
    use crate::raw;

    use super::*;

    #[test]
    fn sample_fixture_matches_record_ids() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .unwrap();
        let raw_path = root.join("fixtures/sample.raw.jsonl");
        let rows = raw::read_raw_capture(&raw_path).unwrap();
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
        assert_eq!(records[0]["source_order"], 0);
        assert_eq!(records[1]["source_order"], 1);

        let mut builder = CaptureRecordBuilder::new("zh-Hant").unwrap();
        let incremental_records = builder
            .build_records(&rows.rows)
            .into_iter()
            .map(|record| record.value)
            .collect::<Vec<_>>();
        assert_eq!(incremental_records.as_slice(), records.as_slice());
    }

    #[test]
    fn fork_protocol_record_id_material_uses_group_draw_index() {
        let first_row = fork_protocol_parsed_row(true, 0);
        let second_row = fork_protocol_parsed_row(false, 0);
        let first_half = fork_protocol_canonical_row(&first_row);
        let second_half = fork_protocol_canonical_row(&second_row);

        assert_eq!(
            record_id_material(&first_half),
            vec![
                "fork",
                "protocol_v2",
                "639175144000000000",
                "ForkLottery_Nanali",
                "0",
                "fork_dustbin",
                "1",
            ]
        );
        assert_eq!(
            record_id_material(&second_half),
            vec![
                "fork",
                "protocol_v2",
                "639175144000000000",
                "ForkLottery_Nanali",
                "5",
                "fork_dustbin",
                "1",
            ]
        );
    }

    #[test]
    fn fork_protocol_record_ids_do_not_depend_on_duplicate_occurrence_order() {
        let first_row = fork_protocol_parsed_row(true, 3);
        let second_row = fork_protocol_parsed_row(false, 3);

        let forward = record_ids(&[
            fork_protocol_canonical_row(&first_row),
            fork_protocol_canonical_row(&second_row),
        ]);
        let reversed = record_ids(&[
            fork_protocol_canonical_row(&second_row),
            fork_protocol_canonical_row(&first_row),
        ]);

        assert_eq!(forward[0], reversed[1]);
        assert_eq!(forward[1], reversed[0]);
        assert_ne!(forward[0], forward[1]);
    }

    #[test]
    fn monopoly_record_id_material_includes_non_numeric_roll_label() {
        let without_label_row = monopoly_protocol_parsed_row(None, None);
        let with_label_row =
            monopoly_protocol_parsed_row(None, Some("BPUI_LotteryResult_jidianzengli".to_string()));
        let without_label = monopoly_protocol_canonical_row(&without_label_row);
        let with_label = monopoly_protocol_canonical_row(&with_label_row);

        assert_ne!(
            record_id_material(&without_label),
            record_id_material(&with_label)
        );
    }

    fn fork_protocol_parsed_row(query_high: bool, row_index: u32) -> ParsedRow {
        ParsedRow {
            record_type: RecordType::Fork,
            ticks: 639_175_144_000_000_000,
            time: Some("2026-06-20T00:00:00.000000".to_string()),
            pool_id: Some("ForkLottery_Nanali".to_string()),
            item_id: "fork_dustbin".to_string(),
            count: 1,
            roll_points: None,
            roll_label_id: None,
            secondary_item_id: None,
            secondary_count: None,
            source: SourceRef {
                session: 0,
                line: 1,
                packet_index: 0,
                view: "shift8:1".to_string(),
                row_index,
                offset: 0,
                stream_key: Some("fork".to_string()),
                page_index: Some(0),
                query_high: Some(query_high),
                segment_index: Some(if query_high { 0 } else { 1 }),
                generation_index: Some(0),
            },
        }
    }

    fn fork_protocol_canonical_row(row: &ParsedRow) -> CanonicalRow<'_> {
        CanonicalRow {
            row,
            item_id: "fork_dustbin".to_string(),
            secondary_item_id: None,
        }
    }

    fn monopoly_protocol_parsed_row(
        roll_points: Option<u32>,
        roll_label_id: Option<String>,
    ) -> ParsedRow {
        ParsedRow {
            record_type: RecordType::Monopoly,
            ticks: 639_175_144_000_000_000,
            time: Some("2026-06-20T00:00:00.000000".to_string()),
            pool_id: Some("CardPool_Character".to_string()),
            item_id: "fork_dustbin".to_string(),
            count: 1,
            roll_points,
            roll_label_id,
            secondary_item_id: None,
            secondary_count: None,
            source: SourceRef {
                session: 0,
                line: 1,
                packet_index: 0,
                view: "shift8:1".to_string(),
                row_index: 0,
                offset: 0,
                stream_key: Some("monopoly".to_string()),
                page_index: Some(0),
                query_high: Some(true),
                segment_index: Some(0),
                generation_index: Some(0),
            },
        }
    }

    fn monopoly_protocol_canonical_row(row: &ParsedRow) -> CanonicalRow<'_> {
        CanonicalRow {
            row,
            item_id: "fork_dustbin".to_string(),
            secondary_item_id: None,
        }
    }
}
