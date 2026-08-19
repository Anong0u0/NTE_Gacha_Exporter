use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    GuiError, InternalRecord, MapData, PoolKind, assign_stable_record_ids, classify_pool_id,
};

pub const PUBLIC_JSON_SCHEMA: &str = "nte-gacha-export";
pub const PUBLIC_JSON_SCHEMA_VERSION: &str = "2.1";

pub fn is_valid_lottery_id(value: &str) -> bool {
    value.strip_prefix("Lottery_").is_some_and(|tail| {
        !tail.is_empty()
            && tail
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    })
}

pub fn public_record_id_from_material(material: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(material.join("\x1f").as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn parse_public_document(
    document_text: &str,
    map: &MapData,
) -> Result<Vec<InternalRecord>, GuiError> {
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
    if !matches!(schema_version, "2.0" | PUBLIC_JSON_SCHEMA_VERSION) {
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
        result.push(parse_record(record, schema_version, map)?);
    }
    assign_stable_record_ids(&mut result);
    Ok(result)
}

fn parse_record(
    value: &Value,
    schema_version: &str,
    map: &MapData,
) -> Result<InternalRecord, GuiError> {
    value
        .as_object()
        .ok_or_else(|| GuiError::InvalidDocument("record must be an object".to_string()))?;
    let record_id = required_text(value, "record_id")?;
    let pool_id = required_text(value, "pool_id")?;
    let pool_kind = classify_pool_id(&pool_id)?;
    let record_type = required_text(value, "record_type")?;
    let expected_record_type = match pool_kind {
        PoolKind::MonopolyLimited | PoolKind::MonopolyStandard => "monopoly",
        PoolKind::ForkLottery => "fork",
    };
    if record_type != expected_record_type {
        return Err(GuiError::InvalidDocument(format!(
            "record_type {record_type} does not match pool_id {pool_id}"
        )));
    }
    let time = optional_text(value, "time");
    let lottery_id = if schema_version == "2.0" {
        map.resolve_legacy_lottery_id(
            &pool_id,
            time.as_deref(),
            optional_text(value, "banner_id").as_deref(),
        )?
    } else if expected_record_type == "monopoly" {
        let lottery_id = required_text(value, "lottery_id")?;
        if !is_valid_lottery_id(&lottery_id) {
            return Err(GuiError::InvalidDocument(format!(
                "invalid lottery_id: {lottery_id}"
            )));
        }
        Some(lottery_id)
    } else {
        None
    };
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
        time,
        pool_id,
        lottery_id,
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
        .filter(|text| !text.trim().is_empty())
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
