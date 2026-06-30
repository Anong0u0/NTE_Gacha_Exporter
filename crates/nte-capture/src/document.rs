use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value, json};

use crate::protocol::{ParsedRow, row_public_time};
use nte_core::{
    GuiError, PUBLIC_JSON_SCHEMA, RecordIdentityInput, compare_time_asc,
    record_semantic_key_from_parts, stable_record_id_from_key,
};
use nte_core::{MapData, load_map};

#[derive(Debug, Clone)]
pub struct CapturePublicRecord {
    pub record_id: String,
    pub record_key: String,
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
    source_index: usize,
    row: &'a ParsedRow,
    banner_id: Option<String>,
    item_id: String,
    rarity: Option<u8>,
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
        let canonical = canonical_row(0, row, &self.map);
        let record_key = record_key(&canonical);
        let record_id = self.next_record_id(&canonical);
        let source_order = self.next_source_order;
        self.next_source_order += 1;
        let value = public_record(&canonical, record_id.clone(), source_order);
        CapturePublicRecord {
            record_id,
            record_key,
            record_type: row.record_type.as_str().to_string(),
            pool_id: row.pool_id.clone(),
            value,
        }
    }

    fn next_record_id(&mut self, row: &CanonicalRow<'_>) -> String {
        let base_id = record_key(row);
        let occurrence = self.seen_record_ids.entry(base_id.clone()).or_default();
        let record_id = stable_record_id_from_key(&base_id, *occurrence);
        *occurrence += 1;
        record_id
    }
}

pub fn build_capture_document(rows: &[ParsedRow], locale: &str) -> Result<Value, GuiError> {
    let map = load_map(locale)?;
    let mut canonical_rows = rows
        .iter()
        .enumerate()
        .map(|(source_index, row)| canonical_row(source_index, row, &map))
        .collect::<Vec<_>>();
    canonical_rows.sort_by(compare_canonical_rows_chronological);
    let record_ids = record_ids(&canonical_rows);
    let records = canonical_rows
        .iter()
        .zip(record_ids)
        .enumerate()
        .map(|(source_order, (row, record_id))| public_record(row, record_id, source_order as u64))
        .collect::<Vec<_>>();
    Ok(json!({
        "info": {
            "schema": PUBLIC_JSON_SCHEMA,
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

fn canonical_row<'a>(source_index: usize, row: &'a ParsedRow, map: &MapData) -> CanonicalRow<'a> {
    let banner = row
        .pool_id
        .as_deref()
        .map(|pool_id| map.resolve_banner(pool_id, row_public_time(row).as_deref()));
    let item_id = map.canonical_item_id(&row.item_id).to_string();
    CanonicalRow {
        source_index,
        row,
        banner_id: banner
            .filter(|banner| banner.resolution_issue.is_none())
            .and_then(|banner| banner.banner_id),
        rarity: map.item_rarity(&item_id),
        item_id,
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
    insert_opt_string(&mut object, "banner_id", row.banner_id.clone());
    insert_string(&mut object, "item_id", row.item_id.clone());
    if let Some(rarity) = row.rarity {
        object.insert("rarity".to_string(), json!(rarity));
    }
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
    let base_ids = rows.iter().map(record_key).collect::<Vec<_>>();
    let mut totals: BTreeMap<String, u64> = BTreeMap::new();
    for id in &base_ids {
        *totals.entry(id.clone()).or_default() += 1;
    }
    let mut seen: BTreeMap<String, u64> = BTreeMap::new();
    let mut ids = Vec::with_capacity(rows.len());
    for base_id in base_ids {
        if totals.get(&base_id) == Some(&1) {
            ids.push(base_id);
            continue;
        }
        let occurrence = seen.entry(base_id.clone()).or_default();
        ids.push(stable_record_id_from_key(&base_id, *occurrence));
        *occurrence += 1;
    }
    ids
}

fn record_key(row: &CanonicalRow<'_>) -> String {
    let secondary_item_id = secondary_item_id(row);
    let secondary_count = secondary_item_id
        .as_ref()
        .and(row.row.secondary_count.map(i64::from));
    record_semantic_key_from_parts(RecordIdentityInput {
        record_type: row.row.record_type.as_str(),
        time: row_public_time(row.row).as_deref(),
        pool_id: row.row.pool_id.as_deref().unwrap_or_default(),
        item_id: &row.item_id,
        count: Some(i64::from(row.row.count)),
        roll_points: row.row.roll_points.map(i64::from),
        roll_label_id: row.row.roll_label_id.as_deref(),
        secondary_item_id: secondary_item_id.as_deref(),
        secondary_count,
    })
}

fn compare_canonical_rows_chronological(
    left: &CanonicalRow<'_>,
    right: &CanonicalRow<'_>,
) -> std::cmp::Ordering {
    compare_time_asc(
        row_public_time(left.row).as_deref(),
        row_public_time(right.row).as_deref(),
    )
    .then_with(|| right.source_index.cmp(&left.source_index))
    .then_with(|| record_key(left).cmp(&record_key(right)))
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

    use crate::protocol::{RecordType, SourceRef};
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
            "02a89041035a12520d0abb8f7d588b5b25b3e946608362b74ac162bf0031a837"
        );
        assert_eq!(records[0]["banner_id"], "monopoly_limited_Nanali");
        assert_eq!(records[0]["rarity"], 5);
        assert_eq!(
            records[1]["record_id"],
            "1634e7929a153facd563ea7e4d9920ac3fd8c02fe64b07c5823d49f0c24dee50"
        );
        assert_eq!(records[1]["banner_id"], "ForkLottery_AnHunQu");
        assert_eq!(records[1]["rarity"], 3);
        assert_eq!(records[0]["source_order"], 0);
        assert_eq!(records[1]["source_order"], 1);

        assert!(
            records
                .iter()
                .enumerate()
                .all(|(index, record)| record["source_order"] == index as u64)
        );
    }

    #[test]
    fn protocol_record_key_ignores_source_slot() {
        let first_row = fork_protocol_parsed_row(true, 0);
        let second_row = fork_protocol_parsed_row(false, 0);
        let first_half = fork_protocol_canonical_row(&first_row);
        let second_half = fork_protocol_canonical_row(&second_row);

        assert_eq!(record_key(&first_half), record_key(&second_half));
        assert_eq!(
            record_ids(&[fork_protocol_canonical_row(&first_row)]),
            record_ids(&[fork_protocol_canonical_row(&second_row)])
        );
    }

    #[test]
    fn protocol_duplicate_record_ids_use_semantic_occurrence() {
        let first_row = fork_protocol_parsed_row(true, 3);
        let second_row = fork_protocol_parsed_row(false, 3);

        let ids = record_ids(&[
            fork_protocol_canonical_row(&first_row),
            fork_protocol_canonical_row(&second_row),
        ]);

        assert_ne!(ids[0], ids[1]);
    }

    #[test]
    fn protocol_record_ids_do_not_reuse_same_id_for_same_semantic_duplicate() {
        let row = fork_protocol_parsed_row(true, 3);

        let ids = record_ids(&[
            fork_protocol_canonical_row(&row),
            fork_protocol_canonical_row(&row),
        ]);

        assert_ne!(ids[0], ids[1]);
    }

    #[test]
    fn capture_document_assigns_source_order_oldest_first() {
        let newest = fork_protocol_parsed_row(true, 0);
        let mut oldest = fork_protocol_parsed_row(false, 0);
        oldest.time = newest.time.clone();
        oldest.item_id = "fork_vine".to_string();
        let document = build_capture_document(&[newest, oldest], "zh-Hant").unwrap();
        let records = document["nte"]["list"].as_array().unwrap();

        assert_eq!(records[0]["source_order"], 0);
        assert_eq!(records[0]["item_id"], "fork_vine");
        assert_eq!(records[1]["source_order"], 1);
        assert_eq!(records[1]["item_id"], "fork_dustbin");
    }

    #[test]
    fn capture_document_canonicalizes_case_folded_fork_item_ids() {
        let mut row = fork_protocol_parsed_row(true, 0);
        row.pool_id = Some("ForkLottery_Nanali".to_string());
        row.item_id = "fork_Wushoutieyu".to_string();
        row.time = Some("2026-05-10T18:15:39.000000".to_string());

        let document = build_capture_document(&[row], "zh-Hant").unwrap();
        let records = document["nte"]["list"].as_array().unwrap();

        assert_eq!(records[0]["item_id"], "fork_wushoutieyu");
        assert_eq!(records[0]["rarity"], 5);
    }

    #[test]
    fn monopoly_record_key_includes_non_numeric_roll_label() {
        let without_label_row = monopoly_protocol_parsed_row(None, None);
        let with_label_row =
            monopoly_protocol_parsed_row(None, Some("BPUI_LotteryResult_jidianzengli".to_string()));
        let without_label = monopoly_protocol_canonical_row(&without_label_row);
        let with_label = monopoly_protocol_canonical_row(&with_label_row);

        assert_ne!(record_key(&without_label), record_key(&with_label));
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
            source_index: 0,
            row,
            banner_id: None,
            item_id: "fork_dustbin".to_string(),
            rarity: None,
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
            source_index: 0,
            row,
            banner_id: None,
            item_id: "fork_dustbin".to_string(),
            rarity: None,
            secondary_item_id: None,
        }
    }
}
