use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, Timelike};
use regex::Regex;
use serde_json::{json, Map, Value};

use nte_core::GuiError;

const MAP_SCHEMA_VERSION: u64 = 4;
const ASSET_FALLBACK_LOCALE: &str = "en";
const REMOVED_MAP_LOCALES: &[&str] = &["en-JM"];

const TABLES: &[(&str, &str)] = &[
    ("inventory", INVENTORY_TABLE),
    ("capital", "DataTable/Inventory/DT_CapitalItemConfig.json"),
    ("fork", "DataTable/Fork/DT_ForkItemData.json"),
    ("character", CHARACTER_TABLE),
];
const ITEM_TYPE_TABLE: &str = "DataTable/Inventory/DT_ItemType.json";
const INVENTORY_TABLE: &str = "DataTable/Inventory/DT_ItemConfig.json";
const CHARACTER_TABLE: &str = "DataTable/Character/DT_Character.json";
const COMBAT_AWARD_TABLE: &str = "DataTable/CombatAward/DT_CombatAwardEntranceConfig.json";
const APPEARANCE_TABLES: &[(&str, &str)] = &[(
    "appearance",
    "DataTable/Character/Appearance/DT_AppearanceData.json",
)];
const VEHICLE_TABLES: &[(&str, &str)] = &[("vehicle", "DataTable/Vehicle/DT_VehicleData.json")];
const VEHICLE_MODULE_TABLES: &[(&str, &str)] = &[(
    "vehicle_module",
    "DataTable/Vehicle/DT_vehicleModuleData.json",
)];
const POOL_TABLES: &[(&str, &str)] = &[("fork_pool", "DataTable/Fork/DT_ForkLotteryPoolData.json")];
const GACHA_ILLUSTRATE_TABLE: &str = "DataTable/Gacha/GachaIllustrate.json";
const BUSINESS_CARD_TABLE: &str = "DataTable/PlayerInfo/DT_BusinessCardConfig.json";
const DROP_GROUP_TABLE: &str = "DataTable/Drop/Client/ClientDropGroupDataTable.json";
const DROP_SEQUENCE_TABLE: &str = "DataTable/Drop/DropSequenceDataTable.json";
const MONOPOLY_TITLE_NAMESPACE: &str = "ST_Ui";
const MONOPOLY_TITLE_PREFIX: &str = "Lottery_Kachimingcheng_";
const MONOPOLY_DESCRIPTION_KEYS: &[&str] = &[
    "LotteryDes_Jishishuoming_{tail}Des",
    "LotteryDes_JIshishuoming_{tail}Des",
];
const STANDARD_MONOPOLY_TITLE_TAIL: &str = "changzhu";
const MONOPOLY_LIMITED_RULE_TEXT_KEY: &str = "LotteryDes_XiandingJishiguize_Des";
const MONOPOLY_STANDARD_RULE_TEXT_KEY: &str = "LotteryDes_Changzhujishiguize_Des";
const MONOPOLY_CELL_TABLE: &str = "DataTable/Monopoly/DT_MonopolyCellDataTable.json";
const FORK_POOL_TABLE: &str = "DataTable/Fork/DT_ForkLotteryPoolData.json";

#[derive(Debug, Clone)]
pub struct AssetMapBuild {
    pub locale: String,
    pub map: Value,
    pub item_count: usize,
    pub pool_count: usize,
    pub label_count: usize,
}

#[derive(Debug, Clone)]
struct LimitedMonopolyBanner {
    banner_id: String,
    title: String,
    start_at_tz8: Option<String>,
    end_at_tz8: Option<String>,
    rate_up_5: Vec<String>,
}

#[derive(Debug, Clone)]
struct ItemRef {
    id: String,
    raw_id: String,
}

#[derive(Debug, Clone)]
struct ItemCanonicalizer {
    item_priorities: BTreeMap<String, i32>,
    by_folded: BTreeMap<String, Vec<String>>,
    signature_aliases: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
struct ItemBuildContext {
    localization: Localization,
    item_type_prefixes: BTreeMap<String, String>,
    canonicalizer: ItemCanonicalizer,
    required_item_ids: BTreeSet<String>,
    item_aliases: BTreeMap<String, String>,
}

type Localization = BTreeMap<String, BTreeMap<String, String>>;
type JsonObject = Map<String, Value>;
type PoolBuildData = (BTreeMap<String, String>, BTreeMap<String, JsonObject>);
