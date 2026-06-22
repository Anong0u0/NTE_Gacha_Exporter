#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordDerived {
    pub record_id: String,
    pub banner_id: Option<String>,
    pub banner_version: Option<String>,
    pub counts_as_pull: bool,
    pub global_pull_no: Option<u64>,
    pub pull_no_in_pool_kind: Option<u64>,
    pub pull_no_in_banner: Option<u64>,
    pub pity_5_before: u64,
    pub pity_5_after: u64,
    pub roll_gift_progress_after: Option<u8>,
    pub hit_rarity: Option<u8>,
    pub rate_up_result: RateUpResult,
    pub guarantee_5_before: Option<bool>,
    pub guarantee_5_after: Option<bool>,
    pub fork_up_pity_before: Option<u64>,
    pub fork_up_pity_after: Option<u64>,
    pub fork_forced_up: Option<bool>,
    pub rule: GachaRuleView,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DisplayRecord {
    pub record_id: String,
    pub source_order: u64,
    pub record_type: String,
    pub time: Option<String>,
    pub pool_kind: PoolKind,
    pub pool_id: String,
    pub pool_label: String,
    pub banner: ResolvedBanner,
    pub item_id: String,
    pub item_name: String,
    pub item_asset_refs: BTreeMap<String, serde_json::Value>,
    pub rarity: Option<u8>,
    pub count: Option<i64>,
    pub roll_points: Option<i64>,
    pub roll_label_id: Option<String>,
    pub roll_label: Option<String>,
    pub secondary_item_id: Option<String>,
    pub secondary_item_name: Option<String>,
    pub secondary_item_asset_refs: BTreeMap<String, serde_json::Value>,
    pub secondary_count: Option<i64>,
    pub derived: RecordDerived,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RecordFilter {
    pub pool_kind: Option<PoolKind>,
    #[serde(default)]
    pub banner_ids: Vec<String>,
    #[serde(default)]
    pub rarities: Vec<u8>,
    #[serde(default)]
    pub hit_rarities: Vec<u8>,
    #[serde(default)]
    pub rate_up_results: Vec<RateUpResult>,
    #[serde(default)]
    pub roll_buckets: Vec<RollBucket>,
    #[serde(default)]
    pub item_kinds: Vec<ItemKind>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub search: Option<String>,
    pub sort_direction: Option<SortDirection>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordList {
    pub total: u64,
    pub records: Vec<DisplayRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordFilterOptions {
    pub banners: Vec<RecordBannerOption>,
    pub roll_buckets: Vec<RecordRollBucketOption>,
    pub item_kinds: Vec<RecordItemKindOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordBannerOption {
    pub banner_id: String,
    pub pool_kind: PoolKind,
    pub title: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordRollBucketOption {
    pub bucket: RollBucket,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordItemKindOption {
    pub item_kind: ItemKind,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MapLocaleList {
    pub locales: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssetsPackManifest {
    pub schema: String,
    pub schema_version: u32,
    pub app_version: String,
    pub map_hash: String,
    pub source_repo: String,
    pub source_commit: String,
    pub format: String,
    pub quality: u8,
    pub file_count: u64,
    pub assets: Vec<AssetsPackAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssetsPackAsset {
    pub asset_ref: String,
    pub kind: String,
    pub source_path: String,
    pub pack_path: String,
    pub width: u32,
    pub height: u32,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssetsPackStatus {
    pub installed: bool,
    pub compatible: bool,
    pub current_app_version: String,
    pub expected_map_hash: String,
    pub installed_app_version: Option<String>,
    pub installed_map_hash: Option<String>,
    pub source_commit: Option<String>,
    pub file_count: u64,
    pub install_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssetsPackPackage {
    pub app_version: String,
    pub map_hash: String,
    pub release_url: String,
    pub asset_name: String,
    pub download_url: String,
    pub manifest_name: String,
    pub manifest_url: String,
    pub sha256: String,
    pub size: u64,
    pub source_commit: String,
    pub file_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssetsPackCheckReport {
    pub current_app_version: String,
    pub expected_map_hash: String,
    pub channel: UpdateChannel,
    pub installed: bool,
    pub compatible: bool,
    pub package: Option<AssetsPackPackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssetsPackInstallReport {
    pub app_version: String,
    pub map_hash: String,
    pub source_commit: String,
    pub file_count: u64,
    pub install_path: String,
}
