use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GuiError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("csv error: {0}")]
    Csv(#[from] csv::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid document: {0}")]
    InvalidDocument(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Profile {
    pub id: i64,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportReport {
    pub profile_id: i64,
    pub run_id: i64,
    pub source_kind: String,
    pub source_path: Option<String>,
    pub records_seen: u64,
    pub records_inserted: u64,
    pub records_skipped: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PoolRule {
    pub pool_id: String,
    pub pool_name: String,
    pub group_label: String,
    #[serde(default)]
    pub pickup_item_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ItemMeta {
    pub item_id: String,
    pub item_name: String,
    pub rarity: u32,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ItemAlias {
    pub alias_id: String,
    pub item_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PoolSummary {
    pub pool_id: String,
    pub pool_name: String,
    pub group_label: String,
    pub record_count: u64,
    pub hit_count: u64,
    pub current_pity: Option<u64>,
    pub last_time: Option<String>,
    pub last_item_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TypeSummary {
    pub record_type: String,
    pub record_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TimelineBucket {
    pub day: String,
    pub record_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LatestRecord {
    pub record_id: String,
    pub record_type: String,
    pub time: Option<String>,
    pub pool_id: Option<String>,
    pub pool_name: Option<String>,
    pub item_id: String,
    pub item_name: Option<String>,
    pub count: Option<i64>,
    pub roll_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DashboardSummary {
    pub profile: Profile,
    pub total_records: u64,
    pub pools: Vec<PoolSummary>,
    pub by_record_type: Vec<TypeSummary>,
    pub timeline: Vec<TimelineBucket>,
    pub latest_records: Vec<LatestRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RecordFilter {
    pub pool_id: Option<String>,
    pub record_type: Option<String>,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredRecord {
    pub record_id: String,
    pub record_type: String,
    pub time: Option<String>,
    pub pool_id: Option<String>,
    pub pool_name: Option<String>,
    pub item_id: String,
    pub item_name: Option<String>,
    pub count: Option<i64>,
    pub roll_points: Option<i64>,
    pub roll_label: Option<String>,
    pub secondary_item_id: Option<String>,
    pub secondary_item_name: Option<String>,
    pub secondary_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordList {
    pub total: u64,
    pub records: Vec<StoredRecord>,
}
