use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use atomic_write_file::AtomicWriteFile;
use serde::Serialize;
use serde_json::{Value, json};

use crate::MapData;
use crate::classify_pool_id;
use crate::display_records;
use crate::{DisplayRecord, GuiError, InternalRecord};

const CSV_FIELDS: [&str; 21] = [
    "time",
    "pool_group",
    "pool_name",
    "item_name",
    "count",
    "roll_label",
    "secondary_item_name",
    "secondary_count",
    "banner_id",
    "banner_name",
    "pull_no",
    "pool_pull_no",
    "pity_5_before",
    "pity_4_before",
    "hit_rarity",
    "rate_up_result",
    "guarantee_5_before",
    "guarantee_5_after",
    "guarantee_4_before",
    "guarantee_4_after",
    "roll_points",
];

pub fn export_public_json(
    path: &Path,
    records: &[InternalRecord],
    map: &MapData,
    locale: &str,
) -> Result<(), GuiError> {
    let display_records = display_records(records, map)?;
    let public_records = display_records
        .iter()
        .map(public_record)
        .collect::<Vec<_>>();
    let document = json!({
        "info": {
            "schema": "nte-gacha-exporter-export",
            "schema_version": "2.0",
            "export_app": "nte-gacha-exporter",
            "export_app_version": env!("CARGO_PKG_VERSION"),
            "export_timestamp": now_stamp(),
            "locale": locale,
            "name_source": "localization_map",
            "time_source": "json_store",
            "privacy": "sanitized"
        },
        "nte": {
            "list": public_records
        }
    });
    write_bytes(path, &serde_json::to_vec_pretty(&document)?)
}

pub fn export_csv(path: &Path, records: &[InternalRecord], map: &MapData) -> Result<(), GuiError> {
    let display_records = display_records(records, map)?;
    let mut lines = Vec::new();
    let headers = CSV_FIELDS
        .iter()
        .map(|field| csv_cell(csv_header(map, field)))
        .collect::<Vec<_>>()
        .join(",");
    lines.push(headers);

    for display in display_records {
        let row = csv_row(&display, map)?;
        lines.push(
            CSV_FIELDS
                .iter()
                .map(|field| csv_cell(row_value(&row, field)))
                .collect::<Vec<_>>()
                .join(","),
        );
    }

    let mut text = lines.join("\n");
    text.push('\n');
    write_bytes(path, text.as_bytes())
}

fn public_record(display: &DisplayRecord) -> Value {
    let mut value = json!({
        "record_id": display.record_id,
        "record_type": display.record_type,
        "pool_id": display.pool_id,
        "pool_name": display.pool_label,
        "item_id": display.item_id,
        "item_name": display.item_name
    });
    let object = value.as_object_mut().expect("json object");
    if let Some(time) = display.time.as_ref() {
        object.insert("time".to_string(), json!(time));
    }
    if let Some(rarity) = display.rarity {
        object.insert("rarity".to_string(), json!(rarity));
    }
    if let Some(count) = display.count {
        object.insert("count".to_string(), json!(count));
    }
    if let Some(roll_points) = display.roll_points {
        object.insert("roll_points".to_string(), json!(roll_points));
        object.insert("roll_label".to_string(), json!(roll_points.to_string()));
    }
    object.insert(
        "banner_resolution_status".to_string(),
        json!(display.banner.status),
    );
    object.insert("pool_kind".to_string(), json!(display.pool_kind));
    object.insert(
        "pull_no_in_pool_kind".to_string(),
        json!(display.derived.pull_no_in_pool_kind),
    );
    object.insert(
        "pity_5_before".to_string(),
        json!(display.derived.pity_5_before),
    );
    object.insert(
        "pity_5_after".to_string(),
        json!(display.derived.pity_5_after),
    );
    object.insert(
        "pity_4_before".to_string(),
        json!(display.derived.pity_4_before),
    );
    object.insert(
        "pity_4_after".to_string(),
        json!(display.derived.pity_4_after),
    );
    object.insert(
        "rate_up_result".to_string(),
        json!(display.derived.rate_up_result),
    );
    object.insert(
        "rule_resolution_status".to_string(),
        json!(display.derived.rule.status),
    );
    if let Some(banner_id) = display.banner.banner_id.as_ref() {
        object.insert("banner_id".to_string(), json!(banner_id));
    }
    if let Some(banner_name) = display.banner.title.as_ref() {
        object.insert("banner_name".to_string(), json!(banner_name));
    }
    if let Some(banner_type) = display.banner.banner_type.as_ref() {
        object.insert("banner_type".to_string(), json!(banner_type));
    }
    if let Some(version) = display.derived.banner_version.as_ref() {
        object.insert("banner_version".to_string(), json!(version));
    }
    if let Some(pull_no) = display.derived.pull_no_in_banner {
        object.insert("pull_no_in_banner".to_string(), json!(pull_no));
    }
    if let Some(rarity) = display.derived.hit_rarity {
        object.insert("hit_rarity".to_string(), json!(rarity));
    }
    if let Some(value) = display.derived.guarantee_5_before {
        object.insert("guarantee_5_before".to_string(), json!(value));
    }
    if let Some(value) = display.derived.guarantee_5_after {
        object.insert("guarantee_5_after".to_string(), json!(value));
    }
    if let Some(value) = display.derived.guarantee_4_before {
        object.insert("guarantee_4_before".to_string(), json!(value));
    }
    if let Some(value) = display.derived.guarantee_4_after {
        object.insert("guarantee_4_after".to_string(), json!(value));
    }
    if let Some(rule_id) = display.derived.rule.rule_id.as_ref() {
        object.insert("rule_id".to_string(), json!(rule_id));
    }
    if let Some(secondary_item_id) = display.secondary_item_id.as_ref() {
        object.insert("secondary_item_id".to_string(), json!(secondary_item_id));
    }
    if let Some(secondary_item_name) = display.secondary_item_name.as_ref() {
        object.insert(
            "secondary_item_name".to_string(),
            json!(secondary_item_name),
        );
    }
    if let Some(secondary_count) = display.secondary_count {
        object.insert("secondary_count".to_string(), json!(secondary_count));
    }
    value
}

struct CsvRow {
    time: String,
    pool_group: String,
    pool_name: String,
    item_name: String,
    count: String,
    roll_label: String,
    secondary_item_name: String,
    secondary_count: String,
    banner_id: String,
    banner_name: String,
    pull_no: String,
    pool_pull_no: String,
    pity_5_before: String,
    pity_4_before: String,
    hit_rarity: String,
    rate_up_result: String,
    guarantee_5_before: String,
    guarantee_5_after: String,
    guarantee_4_before: String,
    guarantee_4_after: String,
    roll_points: String,
}

fn csv_row(record: &DisplayRecord, map: &MapData) -> Result<CsvRow, GuiError> {
    Ok(CsvRow {
        time: record.time.clone().unwrap_or_default(),
        pool_group: map.pool_kind_label(classify_pool_id(&record.pool_id)?),
        pool_name: record.pool_label.clone(),
        item_name: record.item_name.clone(),
        count: record
            .count
            .map(|value| value.to_string())
            .unwrap_or_default(),
        roll_label: record
            .roll_points
            .map(|value| value.to_string())
            .unwrap_or_default(),
        secondary_item_name: record.secondary_item_name.clone().unwrap_or_default(),
        secondary_count: record
            .secondary_count
            .map(|value| value.to_string())
            .unwrap_or_default(),
        banner_id: record.derived.banner_id.clone().unwrap_or_default(),
        banner_name: record.banner.title.clone().unwrap_or_default(),
        pull_no: record
            .derived
            .pull_no_in_banner
            .unwrap_or(record.derived.pull_no_in_pool_kind)
            .to_string(),
        pool_pull_no: record.derived.pull_no_in_pool_kind.to_string(),
        pity_5_before: record.derived.pity_5_before.to_string(),
        pity_4_before: record.derived.pity_4_before.to_string(),
        hit_rarity: record
            .derived
            .hit_rarity
            .map(|value| value.to_string())
            .unwrap_or_default(),
        rate_up_result: json_label(record.derived.rate_up_result)?,
        guarantee_5_before: bool_cell(record.derived.guarantee_5_before),
        guarantee_5_after: bool_cell(record.derived.guarantee_5_after),
        guarantee_4_before: bool_cell(record.derived.guarantee_4_before),
        guarantee_4_after: bool_cell(record.derived.guarantee_4_after),
        roll_points: record
            .roll_points
            .map(|value| value.to_string())
            .unwrap_or_default(),
    })
}

fn row_value<'a>(row: &'a CsvRow, field: &str) -> &'a str {
    match field {
        "time" => &row.time,
        "pool_group" => &row.pool_group,
        "pool_name" => &row.pool_name,
        "item_name" => &row.item_name,
        "count" => &row.count,
        "roll_label" => &row.roll_label,
        "secondary_item_name" => &row.secondary_item_name,
        "secondary_count" => &row.secondary_count,
        "banner_id" => &row.banner_id,
        "banner_name" => &row.banner_name,
        "pull_no" => &row.pull_no,
        "pool_pull_no" => &row.pool_pull_no,
        "pity_5_before" => &row.pity_5_before,
        "pity_4_before" => &row.pity_4_before,
        "hit_rarity" => &row.hit_rarity,
        "rate_up_result" => &row.rate_up_result,
        "guarantee_5_before" => &row.guarantee_5_before,
        "guarantee_5_after" => &row.guarantee_5_after,
        "guarantee_4_before" => &row.guarantee_4_before,
        "guarantee_4_after" => &row.guarantee_4_after,
        "roll_points" => &row.roll_points,
        _ => "",
    }
}

fn json_label<T: Serialize>(value: T) -> Result<String, GuiError> {
    Ok(serde_json::to_value(value)?
        .as_str()
        .unwrap_or_default()
        .to_string())
}

fn bool_cell(value: Option<bool>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

fn csv_header<'a>(map: &'a MapData, field: &'a str) -> &'a str {
    map.csv_headers
        .get(field)
        .map(String::as_str)
        .unwrap_or(field)
}

fn csv_cell(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), GuiError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = AtomicWriteFile::open(path)?;
    file.write_all(bytes)?;
    file.commit()?;
    Ok(())
}

fn now_stamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or_default()
}
