use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::classify_pool_id;
use crate::{GuiError, InternalRecord, assign_stable_record_ids};

pub const PUBLIC_JSON_SCHEMA: &str = "nte-gacha-export";

pub fn public_record_id_from_material(material: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(material.join("\x1f").as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn parse_public_document(document_text: &str) -> Result<Vec<InternalRecord>, GuiError> {
    let document: Value = serde_json::from_str(document_text)?;
    let info = document
        .get("info")
        .and_then(Value::as_object)
        .ok_or_else(|| GuiError::InvalidDocument("expected info object".to_string()))?;
    if info.get("schema").and_then(Value::as_str) != Some(PUBLIC_JSON_SCHEMA) {
        return Err(GuiError::InvalidDocument(format!(
            "info.schema must be {PUBLIC_JSON_SCHEMA}"
        )));
    }
    let schema_version = info
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            GuiError::InvalidDocument("info.schema_version must be a string".to_string())
        })?;
    let schema_major = schema_version.split('.').next();
    if schema_major != Some("2") {
        return Err(GuiError::InvalidDocument(format!(
            "unsupported schema_version: {schema_version}"
        )));
    }

    let records = document
        .get("nte")
        .and_then(|nte| nte.get("list"))
        .and_then(Value::as_array)
        .ok_or_else(|| GuiError::InvalidDocument("expected nte.list array".to_string()))?;

    let mut result = Vec::with_capacity(records.len());
    for record in records {
        result.push(parse_record(record)?);
    }
    assign_stable_record_ids(&mut result);
    Ok(result)
}

fn parse_record(value: &Value) -> Result<InternalRecord, GuiError> {
    value
        .as_object()
        .ok_or_else(|| GuiError::InvalidDocument("record must be an object".to_string()))?;
    let record_id = required_text(value, "record_id")?;
    let pool_id = required_text(value, "pool_id")?;
    classify_pool_id(&pool_id)?;
    let record_type = required_text(value, "record_type")?;
    let item_id = required_text(value, "item_id")?;
    let count = optional_i64(value, "count");
    let roll_points = optional_roll_points(value, "roll_points");
    let roll_label_id = optional_roll_label_id(value);
    let secondary_item_id = optional_text(value, "secondary_item_id");
    let secondary_count = optional_i64(value, "secondary_count");
    Ok(InternalRecord {
        record_id,
        source_order: required_u64(value, "source_order")?,
        record_type,
        time: optional_text(value, "time"),
        pool_id,
        item_id,
        count,
        roll_points,
        roll_label_id,
        secondary_item_id,
        secondary_count,
    })
}

fn required_text(value: &Value, key: &str) -> Result<String, GuiError> {
    optional_text(value, key)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| GuiError::InvalidDocument(format!("record missing string field: {key}")))
}

fn optional_text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn optional_i64(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_i64)
}

fn optional_u64(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}

fn required_u64(value: &Value, key: &str) -> Result<u64, GuiError> {
    optional_u64(value, key)
        .ok_or_else(|| GuiError::InvalidDocument(format!("record missing u64 field: {key}")))
}

fn optional_roll_points(value: &Value, key: &str) -> Option<i64> {
    optional_i64(value, key).filter(|value| !is_roll_point_sentinel(*value))
}

fn optional_roll_label_id(value: &Value) -> Option<String> {
    optional_text(value, "roll_label_id").or_else(|| {
        optional_i64(value, "roll_points")
            .and_then(roll_label_id_from_sentinel)
            .map(str::to_string)
    })
}

fn is_roll_point_sentinel(value: i64) -> bool {
    matches!(value, 0 | 4_294_967_295)
}

fn roll_label_id_from_sentinel(value: i64) -> Option<&'static str> {
    match value {
        0 => Some("BPUI_LotteryResult_jidianzengli"),
        4_294_967_295 => Some("BPUI_LotteryResult_chenmiandi"),
        _ => None,
    }
}
