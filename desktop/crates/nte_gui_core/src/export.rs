use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use atomic_write_file::AtomicWriteFile;
use serde_json::{json, Value};

use crate::analysis::display_record;
use crate::maps::MapData;
use crate::model::{DisplayRecord, GuiError, InternalRecord};
use crate::rules::classify_pool_id;

const CSV_FIELDS: [&str; 8] = [
    "time",
    "pool_group",
    "pool_name",
    "item_name",
    "count",
    "roll_label",
    "secondary_item_name",
    "secondary_count",
];

pub fn export_public_json(
    path: &Path,
    records: &[InternalRecord],
    map: &MapData,
    locale: &str,
) -> Result<(), GuiError> {
    let public_records = records
        .iter()
        .map(|record| public_record(record, map))
        .collect::<Result<Vec<_>, _>>()?;
    let document = json!({
        "info": {
            "schema": "nte-gacha-export",
            "schema_version": "1.0",
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
    let mut lines = Vec::new();
    let headers = CSV_FIELDS
        .iter()
        .map(|field| csv_cell(csv_header(map, field)))
        .collect::<Vec<_>>()
        .join(",");
    lines.push(headers);

    for record in records {
        let display = display_record(record, map)?;
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

fn public_record(record: &InternalRecord, map: &MapData) -> Result<Value, GuiError> {
    let display = display_record(record, map)?;
    let mut value = json!({
        "record_id": display.record_id,
        "record_type": display.record_type,
        "pool_id": display.pool_id,
        "pool_name": display.pool_label,
        "item_id": display.item_id,
        "item_name": display.item_name
    });
    let object = value.as_object_mut().expect("json object");
    if let Some(time) = display.time {
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
    if let Some(secondary_item_id) = display.secondary_item_id {
        object.insert("secondary_item_id".to_string(), json!(secondary_item_id));
    }
    if let Some(secondary_item_name) = display.secondary_item_name {
        object.insert(
            "secondary_item_name".to_string(),
            json!(secondary_item_name),
        );
    }
    if let Some(secondary_count) = display.secondary_count {
        object.insert("secondary_count".to_string(), json!(secondary_count));
    }
    Ok(value)
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
        _ => "",
    }
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
