#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PityBadge {
    #[serde(rename = "fork_up_guarantee")]
    ForkUpGuarantee,
    #[serde(rename = "fork_5star_guarantee", alias = "fork_five_star_guarantee")]
    ForkFiveStarGuarantee,
    #[serde(rename = "fork_4star_guarantee", alias = "fork_four_star_guarantee")]
    ForkFourStarGuarantee,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ForkResultMark {
    Win,
    Guaranteed,
    Lose,
}

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
    pub ten_pull_progress_before: Option<u8>,
    pub ten_pull_progress_after: Option<u8>,
    pub hit_rarity: Option<u8>,
    pub rate_up_result: RateUpResult,
    pub pity_badge: Option<PityBadge>,
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
    pub item_kind: ItemKind,
    pub rarity: Option<u8>,
    pub count: Option<i64>,
    pub roll_points: Option<i64>,
    pub roll_label_id: Option<String>,
    pub roll_label: Option<String>,
    pub roll_bucket: RollBucket,
    pub fork_result_mark: Option<ForkResultMark>,
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
    pub focused_rarities: Vec<u8>,
    #[serde(default)]
    pub rate_up_results: Vec<RateUpResult>,
    #[serde(default)]
    pub roll_buckets: Vec<RollBucket>,
    #[serde(default)]
    pub item_kinds: Vec<ItemKind>,
    #[serde(default)]
    pub fork_result_marks: Vec<ForkResultMark>,
    #[serde(default)]
    pub fork_pity_badges: Vec<PityBadge>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution_issue: Option<BannerResolutionIssue>,
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
    pub label: String,
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
}

pub const ASSETS_PACK_SCHEMA: &str = "nte-gacha-exporter-assets-pack";
pub const ASSETS_PACK_SCHEMA_VERSION: u32 = 2;

pub fn validate_assets_pack_manifest(manifest: &AssetsPackManifest) -> Result<(), GuiError> {
    if manifest.schema != ASSETS_PACK_SCHEMA
        || manifest.schema_version != ASSETS_PACK_SCHEMA_VERSION
    {
        return Err(GuiError::InvalidAssetsPack(format!(
            "assets pack manifest schema must be {ASSETS_PACK_SCHEMA} v{ASSETS_PACK_SCHEMA_VERSION}"
        )));
    }
    if manifest.format != "webp" {
        return Err(GuiError::InvalidAssetsPack(
            "assets pack format must be webp".to_string(),
        ));
    }
    if manifest.file_count != manifest.assets.len() as u64 {
        return Err(GuiError::InvalidAssetsPack(
            "assets pack file_count mismatch".to_string(),
        ));
    }
    for asset in &manifest.assets {
        if asset.asset_ref.trim().is_empty()
            || asset.kind.trim().is_empty()
            || asset.source_path.trim().is_empty()
            || asset.pack_path.trim().is_empty()
        {
            return Err(GuiError::InvalidAssetsPack(
                "assets pack asset fields must be non-empty".to_string(),
            ));
        }
        if !asset.pack_path.starts_with("assets/")
            || !asset.pack_path.ends_with(".webp")
            || asset.pack_path.contains('\\')
            || asset.pack_path.contains("..")
        {
            return Err(GuiError::InvalidAssetsPack(format!(
                "assets pack contains invalid asset path: {}",
                asset.pack_path
            )));
        }
    }
    Ok(())
}
