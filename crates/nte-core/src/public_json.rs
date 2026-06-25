use std::collections::BTreeMap;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::classify_pool_id;
use crate::{GuiError, InternalRecord};

const LEGACY_RECORD_ID_PARTS: usize = 9;
pub const PUBLIC_JSON_SCHEMA: &str = "nte-gacha-export";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct LegacyRecordId {
    record_type: String,
    ticks: String,
    pool_id: String,
    row_index: u32,
    roll_points: Option<i64>,
    item_id: String,
    count: String,
    secondary_item_id: Option<String>,
    secondary_count: Option<String>,
}

#[derive(Debug, Default)]
struct LegacyIdState {
    seen_base_ids: BTreeMap<String, u64>,
    fork_row_occurrences: BTreeMap<(String, String, u32), u32>,
}

struct RecordIdContext<'a> {
    raw_record_id: &'a str,
    record_type: &'a str,
    pool_id: &'a str,
    item_id: &'a str,
    count: Option<i64>,
    roll_points: Option<i64>,
    roll_label_id: Option<&'a str>,
    secondary_item_id: Option<&'a str>,
    secondary_count: Option<i64>,
}

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
    let mut legacy_state = LegacyIdState::default();
    for record in records {
        result.push(parse_record(record, &mut legacy_state)?);
    }
    Ok(result)
}

fn parse_record(
    value: &Value,
    legacy_state: &mut LegacyIdState,
) -> Result<InternalRecord, GuiError> {
    value
        .as_object()
        .ok_or_else(|| GuiError::InvalidDocument("record must be an object".to_string()))?;
    let raw_record_id = required_text(value, "record_id")?;
    let pool_id = required_text(value, "pool_id")?;
    classify_pool_id(&pool_id)?;
    let record_type = required_text(value, "record_type")?;
    let item_id = required_text(value, "item_id")?;
    let count = optional_i64(value, "count");
    let roll_points = optional_roll_points(value, "roll_points");
    let roll_label_id = optional_roll_label_id(value);
    let secondary_item_id = optional_text(value, "secondary_item_id");
    let secondary_count = optional_i64(value, "secondary_count");
    let record_id = normalize_record_id(
        RecordIdContext {
            raw_record_id: &raw_record_id,
            record_type: &record_type,
            pool_id: &pool_id,
            item_id: &item_id,
            count,
            roll_points,
            roll_label_id: roll_label_id.as_deref(),
            secondary_item_id: secondary_item_id.as_deref(),
            secondary_count,
        },
        legacy_state,
    );
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

fn normalize_record_id(context: RecordIdContext<'_>, legacy_state: &mut LegacyIdState) -> String {
    let Some(legacy) = parse_legacy_record_id(context.raw_record_id) else {
        return context.raw_record_id.to_string();
    };
    if legacy.record_type != context.record_type
        || legacy.pool_id != context.pool_id
        || legacy.item_id != context.item_id
    {
        return context.raw_record_id.to_string();
    }
    if context
        .count
        .is_some_and(|value| value.to_string() != legacy.count)
    {
        return context.raw_record_id.to_string();
    }
    if context
        .roll_points
        .is_some_and(|value| Some(value) != legacy.roll_points)
    {
        return context.raw_record_id.to_string();
    }
    if context
        .secondary_item_id
        .is_some_and(|value| Some(value) != legacy.secondary_item_id.as_deref())
    {
        return context.raw_record_id.to_string();
    }
    if context
        .secondary_count
        .is_some_and(|value| legacy.secondary_count.as_deref() != Some(value.to_string().as_str()))
    {
        return context.raw_record_id.to_string();
    }
    if context.record_type != "fork"
        && context.roll_points.is_none()
        && legacy
            .roll_points
            .is_some_and(|value| !is_roll_point_sentinel(value))
    {
        return context.raw_record_id.to_string();
    }

    let material = if context.record_type == "fork" {
        let group_draw_index = legacy_fork_group_draw_index(&legacy, legacy_state);
        vec![
            context.record_type.to_string(),
            "protocol_v2".to_string(),
            legacy.ticks,
            context.pool_id.to_string(),
            group_draw_index.to_string(),
            context.item_id.to_string(),
            context
                .count
                .map(|value| value.to_string())
                .unwrap_or_else(|| legacy.count.clone()),
        ]
    } else {
        let mut material = vec![
            context.record_type.to_string(),
            legacy.ticks,
            context.pool_id.to_string(),
            legacy.row_index.to_string(),
            context
                .roll_points
                .map(|value| value.to_string())
                .or_else(|| {
                    legacy
                        .roll_points
                        .filter(|value| !is_roll_point_sentinel(*value))
                        .map(|value| value.to_string())
                })
                .unwrap_or_default(),
            context.item_id.to_string(),
            context
                .count
                .map(|value| value.to_string())
                .unwrap_or_else(|| legacy.count.clone()),
            context
                .secondary_item_id
                .or(legacy.secondary_item_id.as_deref())
                .unwrap_or_default()
                .to_string(),
            context
                .secondary_count
                .map(|value| value.to_string())
                .or(legacy.secondary_count.clone())
                .unwrap_or_default(),
        ];
        if let Some(roll_label_id) = context.roll_label_id {
            material.push("roll_label_id".to_string());
            material.push(roll_label_id.to_string());
        }
        material
    };

    let base_id = public_record_id_from_material(&material);
    let occurrence = legacy_state
        .seen_base_ids
        .entry(base_id.clone())
        .or_default();
    let record_id = if *occurrence == 0 {
        base_id
    } else {
        let mut material = material;
        material.push("duplicate_occurrence".to_string());
        material.push(occurrence.to_string());
        public_record_id_from_material(&material)
    };
    *occurrence += 1;
    record_id
}

fn legacy_fork_group_draw_index(legacy: &LegacyRecordId, legacy_state: &mut LegacyIdState) -> u32 {
    let key = (
        legacy.ticks.clone(),
        legacy.pool_id.clone(),
        legacy.row_index,
    );
    let occurrence = legacy_state.fork_row_occurrences.entry(key).or_default();
    let group_draw_index = legacy.row_index + (*occurrence * 5);
    *occurrence += 1;
    group_draw_index
}

fn parse_legacy_record_id(value: &str) -> Option<LegacyRecordId> {
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() != LEGACY_RECORD_ID_PARTS {
        return None;
    }
    let row_index = parts[3].parse::<u32>().ok()?;
    Some(LegacyRecordId {
        record_type: parts[0].to_string(),
        ticks: parts[1].to_string(),
        pool_id: parts[2].to_string(),
        row_index,
        roll_points: parse_optional_i64_part(parts[4]),
        item_id: parts[5].to_string(),
        count: parts[6].to_string(),
        secondary_item_id: optional_part(parts[7]),
        secondary_count: optional_part(parts[8]),
    })
}

fn parse_optional_i64_part(value: &str) -> Option<i64> {
    (!value.is_empty())
        .then(|| value.parse::<i64>().ok())
        .flatten()
}

fn optional_part(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_string())
}
