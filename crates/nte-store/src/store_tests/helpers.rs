use std::io::Write;

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
