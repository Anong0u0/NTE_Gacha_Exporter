#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InternalRecord {
    pub record_id: String,
    pub record_type: String,
    pub time: Option<String>,
    pub pool_id: String,
    pub item_id: String,
    pub count: Option<i64>,
    pub roll_points: Option<i64>,
    pub secondary_item_id: Option<String>,
    pub secondary_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DashboardOverview {
    pub profile: Profile,
    pub last_run: Option<ImportReport>,
    pub total_records: u64,
    pub pool_kinds: Vec<PoolKindSummary>,
    pub banners: Vec<BannerSummary>,
    pub time_stats: TimeStats,
    pub rarity_distribution: Vec<RarityBucket>,
    pub item_ranking: Vec<ItemRank>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BannerSummary {
    pub banner_id: String,
    pub pool_id: String,
    pub pool_kind: PoolKind,
    pub banner_type: Option<String>,
    pub title: String,
    pub version: Option<String>,
    pub start_at: Option<String>,
    pub end_at: Option<String>,
    pub asset_refs: BTreeMap<String, serde_json::Value>,
    pub total_pulls: u64,
    pub roll_points_total: i64,
    pub known_roll_point_records: u64,
    pub missing_roll_point_records: u64,
    pub five_star_count: u64,
    pub four_star_count: u64,
    pub current_5star_pity: u64,
    pub current_4star_pity: u64,
    pub average_5star_pity: Option<f64>,
    pub average_4star_pity: Option<f64>,
    pub rate_up_5_count: u64,
    pub off_rate_5_count: u64,
    pub not_applicable_rate_up_5_count: u64,
    pub unknown_rate_up_5_count: u64,
    pub rate_up_4_count: u64,
    pub off_rate_4_count: u64,
    pub not_applicable_rate_up_4_count: u64,
    pub unknown_rate_up_4_count: u64,
    pub average_roll_points_to_5star: Option<f64>,
    pub average_roll_points_to_4star: Option<f64>,
    pub roll_point_cost_samples_5star: u64,
    pub roll_point_cost_samples_4star: u64,
    pub latest_hit: Option<DisplayRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimeStats {
    pub monthly: Vec<TimeBucketSummary>,
    pub daily: Vec<TimeBucketSummary>,
    pub missing_time_records: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimeBucketSummary {
    pub bucket: String,
    pub total_pulls: u64,
    pub five_star_count: u64,
    pub four_star_count: u64,
    pub roll_points_total: i64,
    pub known_roll_point_records: u64,
    pub missing_roll_point_records: u64,
    pub average_5star_pity: Option<f64>,
    pub average_4star_pity: Option<f64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RecordSortKey {
    #[default]
    Time,
    Pool,
    Item,
    Rarity,
    RecordType,
    Banner,
    PullNo,
    #[serde(rename = "pity_5")]
    Pity5,
    #[serde(rename = "pity_4")]
    Pity4,
    RateUp,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    Asc,
    #[default]
    Desc,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PoolKindSummary {
    pub pool_kind: PoolKind,
    pub label: String,
    pub total_pulls: u64,
    pub roll_points_total: i64,
    pub known_roll_point_records: u64,
    pub missing_roll_point_records: u64,
    pub hit_count: u64,
    pub current_pity: u64,
    pub current_guarantee: bool,
    pub hard_pity: u64,
    pub average_5star_pity: Option<f64>,
    pub min_5star_pity: Option<u64>,
    pub max_5star_pity: Option<u64>,
    pub early_hit_count: u64,
    pub up_count: u64,
    pub off_rate_count: u64,
    pub not_applicable_rate_up_count: u64,
    pub unknown_rate_up_count: u64,
    pub observed_up_rate: Option<f64>,
    pub latest_5star: Option<DisplayRecord>,
    pub current_4star_pity: u64,
    pub hard_pity_4: Option<u64>,
    pub average_4star_pity: Option<f64>,
    pub min_4star_pity: Option<u64>,
    pub max_4star_pity: Option<u64>,
    pub four_star_count: u64,
    pub rate_up_4_count: u64,
    pub off_rate_4_count: u64,
    pub not_applicable_rate_up_4_count: u64,
    pub unknown_rate_up_4_count: u64,
    pub rule_resolution_status: RuleResolutionStatus,
    pub average_roll_points_to_5star: Option<f64>,
    pub average_roll_points_to_4star: Option<f64>,
    pub roll_point_cost_samples_5star: u64,
    pub roll_point_cost_samples_4star: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PoolKindDetail {
    pub summary: PoolKindSummary,
    pub five_star_history: Vec<FiveStarRecord>,
    pub four_star_history: Vec<FourStarRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DashboardSelection {
    PoolKind { pool_kind: PoolKind },
    Banner {
        pool_kind: PoolKind,
        banner_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DashboardSelectionDetail {
    pub summary: PoolKindSummary,
    pub five_star_history: Vec<FiveStarRecord>,
    pub four_star_history: Vec<FourStarRecord>,
    pub rarity_distribution: Vec<RarityBucket>,
    pub item_ranking: Vec<ItemRank>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FiveStarRecord {
    pub record: DisplayRecord,
    pub pity_distance: u64,
    pub result: FiveStarResult,
    pub guarantee_before: Option<bool>,
    pub guarantee_after: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FourStarRecord {
    pub record: DisplayRecord,
    pub pity_distance: u64,
    pub result: RateUpResult,
    pub guarantee_before: Option<bool>,
    pub guarantee_after: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FiveStarResult {
    Up,
    OffRate,
    NotApplicable,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RarityBucket {
    pub rarity: u8,
    pub count: u64,
    pub percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ItemRank {
    pub item_id: String,
    pub item_name: String,
    pub rarity: Option<u8>,
    pub count: u64,
}
