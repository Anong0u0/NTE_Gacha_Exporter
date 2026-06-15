use serde_json::Value;

use crate::model::{GuiError, InternalRecord};
use crate::rules::classify_pool_id;

pub fn parse_public_document(document_text: &str) -> Result<Vec<InternalRecord>, GuiError> {
    let document: Value = serde_json::from_str(document_text)?;
    let info = document
        .get("info")
        .and_then(Value::as_object)
        .ok_or_else(|| GuiError::InvalidDocument("expected info object".to_string()))?;
    if info.get("schema").and_then(Value::as_str) != Some("nte-gacha-export") {
        return Err(GuiError::InvalidDocument(
            "info.schema must be nte-gacha-export".to_string(),
        ));
    }
    let schema_version = info
        .get("schema_version")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            GuiError::InvalidDocument("info.schema_version must be a string".to_string())
        })?;
    if schema_version.split('.').next() != Some("1") {
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
    Ok(result)
}

fn parse_record(value: &Value) -> Result<InternalRecord, GuiError> {
    let object = value
        .as_object()
        .ok_or_else(|| GuiError::InvalidDocument("record must be an object".to_string()))?;
    let pool_id = required_text(value, "pool_id")?;
    classify_pool_id(&pool_id)?;
    Ok(InternalRecord {
        record_id: required_text(value, "record_id")?,
        record_type: required_text(value, "record_type")?,
        time: optional_text(value, "time"),
        pool_id,
        item_id: required_text(value, "item_id")?,
        count: optional_i64(value, "count"),
        roll_points: optional_i64(value, "roll_points"),
        secondary_item_id: optional_text(value, "secondary_item_id"),
        secondary_count: optional_i64(value, "secondary_count"),
    })
    .and_then(|record| {
        if object.contains_key("pool_kind") {
            return Err(GuiError::InvalidDocument(
                "record must not include pool_kind".to_string(),
            ));
        }
        Ok(record)
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
