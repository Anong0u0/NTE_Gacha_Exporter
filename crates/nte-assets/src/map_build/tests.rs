#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_map_from_minimal_assets() {
        let tmp = tempfile::tempdir().unwrap();
        write_minimal_assets(tmp.path());

        let map = build_asset_map(tmp.path(), "zh-Hant").unwrap();

        assert_eq!(map["schema_version"], 4);
        assert_eq!(map["items"]["1010"]["name"], "Character·Nanali");
        assert_eq!(map["items"]["1010"]["rarity"], 5);
        assert_eq!(map["items"]["1010"]["asset_refs"]["banner"], "/Game/Banner");
        assert_eq!(map["pools"]["CardPool_NewRole"]["title"], "Standard Title");
        assert_eq!(
            map["pools"]["ForkLottery_Test"]["banner_ids"][0],
            "ForkLottery_Test"
        );
        assert_eq!(
            map["banners"]["monopoly_limited_Nanali"]["phase"],
            "limited_2026_05_13"
        );
        assert_eq!(
            map["gacha_rules"]["fork_lottery_s"]["hard_pity_5"],
            json!(80)
        );
        assert_eq!(map["labels"]["UW_LotteryBase_BP_Hupanyanmu"], "Fork Group");
    }

    #[test]
    fn discovers_locales_and_skips_removed_locale() {
        let tmp = tempfile::tempdir().unwrap();
        write_json(tmp.path().join("DataTable/.keep"), json!({}));
        fs::create_dir_all(tmp.path().join("DataTable")).unwrap();
        for locale in ["en", "zh-Hant", "en-JM"] {
            write_json(
                tmp.path()
                    .join("Localization")
                    .join(locale)
                    .join("game.json"),
                json!({}),
            );
        }

        let locales = discover_asset_locales(tmp.path()).unwrap();

        assert_eq!(locales, vec!["en", "zh-Hant"]);
    }

    fn write_minimal_assets(root: &Path) {
        write_json(
            root.join("Localization/en/game.json"),
            json!({
                "ST_Common": {
                    "item_type_2": "Item",
                    "item_type_3": "Character",
                    "item_type_5": "Arc",
                    "item_type_10": "Mod Parts"
                },
                "ST_Ui": {
                    "BPUI_LotteryDiceRecord_biaozhunqipan": "Standard",
                    "BPUI_LotteryDiceRecord_xiandingqipan": "Limited",
                    "UW_LotteryBase_BP_Hupanyanmu": "Fork Group",
                    "Lottery_Kachimingcheng_changzhu": "Standard Title",
                    "Lottery_Kachimingcheng_nanali": "Nanali Banner",
                    "BPUI_GashaponRecord_time": "Time",
                    "BPUI_LotteryDiceRecord_qipanleixing": "Pool Type",
                    "BPUI_LotteryDiceRecord_daojumingcheng": "Item Name",
                    "BPUI_ConsumableUse_UseNumber": "Count",
                    "BPUI_LotteryDiceRecord_touzhidianshu": "Roll",
                    "BPUI_LotteryResult_AdditionalReward": "Extra"
                },
                "ST_UI_C": {
                    "BPUI_CharacterEquipDevFilter_15": "Type"
                }
            }),
        );
        write_json(
            root.join("Localization/zh-Hant/game.json"),
            json!({
                "ST_Common": {
                    "item_type_3": "Character",
                    "item_type_5": "Arc"
                },
                "ST_Ui": {
                    "BPUI_LotteryDiceRecord_biaozhunqipan": "標準棋盤",
                    "BPUI_LotteryDiceRecord_xiandingqipan": "限定棋盤",
                    "UW_LotteryBase_BP_Hupanyanmu": "Fork Group",
                    "Lottery_Kachimingcheng_changzhu": "Standard Title",
                    "Lottery_Kachimingcheng_nanali": "Nanali Banner"
                }
            }),
        );
        write_json(
            root.join("DataTable/Character/DT_Character.json"),
            json!({"Rows": {
                "1010": {
                    "ItemName": {"Key": "char_1010", "TableId": "ST_Item"},
                    "ItemQuality": "EItemQuality::ITEM_QUALITY_ORANGE",
                    "ItemIcon": {"AssetPathName": "/Game/Icon"},
                    "ItemIconBig": {"AssetPathName": "/Game/Portrait"}
                }
            }}),
        );
        write_json(
            root.join("DataTable/Fork/DT_ForkItemData.json"),
            json!({"Rows": {
                "200": {
                    "ItemName": {"Key": "fork_200", "TableId": "ST_Item"},
                    "ItemQuality": "EItemQuality::ITEM_QUALITY_ORANGE"
                }
            }}),
        );
        write_json(
            root.join("Localization/zh-Hant/game.json"),
            json!({
                "ST_Common": {
                    "item_type_3": "Character",
                    "item_type_5": "Arc"
                },
                "ST_Item": {
                    "char_1010": "Nanali",
                    "fork_200": "Fork Weapon"
                },
                "ST_Ui": {
                    "BPUI_LotteryDiceRecord_biaozhunqipan": "標準棋盤",
                    "BPUI_LotteryDiceRecord_xiandingqipan": "限定棋盤",
                    "UW_LotteryBase_BP_Hupanyanmu": "Fork Group",
                    "Lottery_Kachimingcheng_changzhu": "Standard Title",
                    "Lottery_Kachimingcheng_nanali": "Nanali Banner"
                }
            }),
        );
        write_json(
            root.join("DataTable/Gacha/DT_LotteryDataTable_Nanali.json"),
            json!({"Rows": {
                "row": {
                    "SSRItems": [{"ItemID": "1010"}],
                    "SRItems": []
                }
            }}),
        );
        write_json(
            root.join("DataTable/Gacha/GachaIllustrate.json"),
            json!({"Rows": {
                "1010": {
                    "ItemIcon": {"AssetPathName": "/Game/Portrait"},
                    "ActivityHeadIcon": {"AssetPathName": "/Game/Banner"},
                    "OutlineColor": {"Hex": "ffcc00"}
                }
            }}),
        );
        write_json(
            root.join("DataTable/Fork/DT_ForkLotteryPoolData.json"),
            json!({"Rows": {
                "ForkLottery_Test": {
                    "Name": "Fork Pool",
                    "ShowText1": "Fork Banner",
                    "UpList": ["200"],
                    "BaseDropID": "drop_fork",
                    "UpGuaranteeCnt": 80,
                    "CurrencyID": "1010",
                    "CurrencyCnt": 1,
                    "OnceLotteryCnt": 1
                }
            }}),
        );
    }

    fn write_json(path: PathBuf, value: Value) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    }
}
