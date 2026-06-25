#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_map_from_minimal_assets() {
        let tmp = tempfile::tempdir().unwrap();
        write_minimal_assets(tmp.path());

        let map = build_asset_map(tmp.path(), "zh-Hant").unwrap();

        assert_eq!(map["schema_version"], 2);
        assert_eq!(map["items"]["1010"]["name"], "Character·Nanali");
        assert_eq!(map["items"]["1010"]["rarity"], 5);
        assert_eq!(map["items"]["201"]["name"], "Arc·Forgotten");
        assert_eq!(map["items"]["201"]["rarity"], 4);
        assert_eq!(
            map["banners"]["monopoly_standard"]["standard_5_pool"],
            json!([
                "1010",
                "201",
                "Fashion_Glide_1010",
                "Fashion_vehicle_1010_V008",
                "Fashion_vehicleSkin_1010_V001"
            ])
        );
        let nanali_refs = map["items"]["1010"]["asset_refs"].as_object().unwrap();
        assert_eq!(nanali_refs.get("head_icon"), Some(&json!("/Game/SmallIcon")));
        assert_eq!(
            nanali_refs.get("banner"),
            Some(&json!(
                "/Game/UI/UI/PlayerInfo/BusinessCards/Card_Small/YH_UI_bg_card_show_strip_08_s.YH_UI_bg_card_show_strip_08_s"
            ))
        );
        assert!(!nanali_refs.contains_key("portrait"));
        assert!(!nanali_refs.contains_key("icon"));
        assert!(!nanali_refs.contains_key("material"));
        assert_eq!(
            map["banners"]["monopoly_limited_Nanali"]["asset_refs"]["image"],
            "/Game/UI/UI/PlayerInfo/BusinessCards/Card_Small/YH_UI_bg_card_show_strip_08_s.YH_UI_bg_card_show_strip_08_s"
        );
        assert!(map["banners"]["monopoly_limited_Nanali"]["asset_refs"]["featured_portraits"].is_null());
        let forgotten_refs = map["items"]["201"]["asset_refs"].as_object().unwrap();
        assert_eq!(forgotten_refs.get("icon"), Some(&json!("/Game/ForkIcon")));
        assert_eq!(
            forgotten_refs.get("head_icon"),
            Some(&json!("/Game/ForkSmallIcon"))
        );
        assert_eq!(forgotten_refs.get("banner"), Some(&json!("/Game/ForkBanner")));
        assert_eq!(
            forgotten_refs.get("material"),
            Some(&json!("/Game/ForkMaterial"))
        );
        let glide_refs = map["items"]["Fashion_Glide_1010"]["asset_refs"]
            .as_object()
            .unwrap();
        assert_eq!(glide_refs.get("icon"), Some(&json!("/Game/GlideIcon")));
        assert!(!glide_refs.contains_key("portrait"));
        assert!(!glide_refs.contains_key("banner"));
        let vehicle_refs = map["items"]["Fashion_vehicle_1010_V008"]["asset_refs"]
            .as_object()
            .unwrap();
        assert_eq!(vehicle_refs.get("icon"), Some(&json!("/Game/VehicleIcon")));
        assert_eq!(
            vehicle_refs.get("head_icon"),
            Some(&json!("/Game/VehicleHead"))
        );
        assert!(!vehicle_refs.contains_key("portrait"));
        let vehicle_skin_refs = map["items"]["Fashion_vehicleSkin_1010_V001"]["asset_refs"]
            .as_object()
            .unwrap();
        assert_eq!(
            vehicle_skin_refs.get("icon"),
            Some(&json!("/Game/VehicleSkinIcon"))
        );
        assert!(!vehicle_skin_refs.contains_key("portrait"));
        assert_eq!(map["pools"]["CardPool_NewRole"]["title"], "Standard Title");
        assert_eq!(
            map["pools"]["ForkLottery_Test"]["banner_ids"][0],
            "ForkLottery_Test"
        );
        assert!(map["pools"]["ForkLottery_Test"]["asset_refs"].is_null());
        assert_eq!(
            map["banners"]["ForkLottery_Test"]["asset_refs"]["image"],
            "/Game/ForkUpIcon"
        );
        assert!(map["banners"]["ForkLottery_Test"]["asset_refs"]["background"].is_null());
        assert!(map["banners"]["ForkLottery_Test"]["asset_refs"]["icon"].is_null());
        assert_eq!(
            map["banners"]["monopoly_limited_Nanali"]["end_at"],
            json!("2026-05-13 05:59:00")
        );
        assert_eq!(
            map["gacha_rules"]["fork_lottery_s"]["hard_pity_5"],
            json!(60)
        );
        assert_eq!(
            map["gacha_rules"]["fork_lottery_s"]["hard_up_pity_5"],
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
                    "item_type_10": "Mod Parts",
                    "item_type_8": "Appearance"
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
                    "BPUI_CharacterEquipDevFilter_15": "Type",
                    "ui_appearance_02": "Glider"
                }
            }),
        );
        write_json(
            root.join("Localization/zh-Hant/game.json"),
            json!({
                "ST_Common": {
                    "item_type_3": "Character",
                    "item_type_5": "Arc",
                    "item_type_8": "Appearance",
                    "item_type_10": "Mod Parts"
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
                    "ItemIcon": {"AssetPathName": "/Game/Portrait"},
                    "ItemIconSmall": {"AssetPathName": "/Game/SmallIcon"},
                    "ItemIconBig": {"AssetPathName": "/Game/Portrait"}
                }
            }}),
        );
        write_json(
            root.join("DataTable/Fork/DT_ForkItemData.json"),
            json!({"Rows": {
                "200": {
                    "ItemName": {"Key": "fork_200", "TableId": "ST_Item"},
                    "ItemQuality": "EItemQuality::ITEM_QUALITY_ORANGE",
                    "ItemIcon": {"AssetPathName": "/Game/ForkUpIcon"}
                },
                "201": {
                    "ItemName": {"Key": "fork_201", "TableId": "ST_Item"},
                    "ItemQuality": "EItemQuality::ITEM_QUALITY_PURPLE",
                    "ItemIcon": {"AssetPathName": "/Game/ForkIcon"},
                    "ItemIconSmall": {"AssetPathName": "/Game/ForkSmallIcon"}
                }
            }}),
        );
        write_json(
            root.join("DataTable/Inventory/DT_ItemConfig.json"),
            json!({"Rows": {
                "Fashion_vehicle_1010_V008": {
                    "ItemName": {"Key": "vehicle_1010", "TableId": "ST_Item"},
                    "ItemQuality": "EItemQuality::ITEM_QUALITY_ORANGE"
                },
                "Fashion_vehicleSkin_1010_V001": {
                    "ItemName": {"Key": "vehicle_skin_1010", "TableId": "ST_Item"},
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
                    "fork_200": "Fork Weapon",
                    "fork_201": "Forgotten",
                    "glide_1010": "Nanali Glide",
                    "vehicle_1010": "Nanali Vehicle",
                    "vehicle_skin_1010": "Nanali Vehicle Skin"
                },
                "ST_Ui": {
                    "BPUI_LotteryDiceRecord_biaozhunqipan": "標準棋盤",
                    "BPUI_LotteryDiceRecord_xiandingqipan": "限定棋盤",
                    "UW_LotteryBase_BP_Hupanyanmu": "Fork Group",
                    "Lottery_Kachimingcheng_changzhu": "Standard Title",
                    "Lottery_Kachimingcheng_nanali": "Nanali Banner",
                    "ui_appearance_02": "Glider"
                }
            }),
        );
        write_json(
            root.join("DataTable/PlayerInfo/DT_BusinessCardConfig.json"),
            json!({"Rows": {
                "1010": {
                    "UnlockDescription": {"Key": "likeability_card_1010"},
                    "IconSource": {
                        "AssetPathName": "/Game/UI/UI/PlayerInfo/BusinessCards/Card_Small/YH_UI_bg_card_show_strip_08_s.YH_UI_bg_card_show_strip_08_s"
                    }
                }
            }}),
        );
        write_json(
            root.join("DataTable/Gacha/DT_LotteryDataTable_Nanali.json"),
            json!({"Rows": {
                "row": {
                    "SSRItems": [
                        {"ItemID": "1010"},
                        {"ItemID": "201"},
                        {"ItemID": "Fashion_Glide_1010"},
                        {"ItemID": "Fashion_vehicle_1010_V008"},
                        {"ItemID": "Fashion_vehicleSkin_1010_V001"}
                    ],
                    "SRItems": []
                }
            }}),
        );
        write_json(
            root.join("DataTable/Character/Appearance/DT_AppearanceData.json"),
            json!({"Rows": {
                "Fashion_Glide_1010": {
                    "Name": {"Key": "glide_1010", "TableId": "ST_Item"},
                    "Quality": "EItemQuality::ITEM_QUALITY_ORANGE",
                    "AppearanceType": "EAppearanceType::Glide",
                    "DisplayIcon": {"AssetPathName": "/Game/GlideIcon"},
                    "HeadIconBig": {"AssetPathName": "/Game/GlidePortrait"},
                    "PortraitImg": {"AssetPathName": "/Game/GlideBanner"}
                }
            }}),
        );
        write_json(
            root.join("DataTable/Vehicle/DT_vehicleModuleData.json"),
            json!({"Rows": {
                "vehicle_module": {
                    "ModuleName": {"Key": "vehicle_1010", "TableId": "ST_Item"},
                    "UnLockNormalIcon": {"AssetPathName": "/Game/VehicleIcon"},
                    "UnLockSelectedIcon": {"AssetPathName": "/Game/VehicleHead"},
                    "NameplateBg": {"AssetPathName": "/Game/VehiclePortrait"},
                    "FeatureActiveData": {
                        "Requires": [
                            {"ID": "Fashion_vehicle_1010_V008"}
                        ]
                    }
                },
                "vehicle_skin_module": {
                    "ModuleName": {"Key": "vehicle_skin_1010", "TableId": "ST_Item"},
                    "UnLockNormalIcon": {"AssetPathName": "/Game/VehicleSkinIcon"},
                    "NameplateBg": {"AssetPathName": "/Game/VehicleSkinPortrait"},
                    "FeatureActiveData": {
                        "Requires": [
                            {"ID": "Fashion_vehicleSkin_1010_V001"}
                        ]
                    }
                }
            }}),
        );
        write_json(
            root.join("DataTable/Gacha/GachaIllustrate.json"),
            json!({"Rows": {
                "1010": {
                    "ItemIcon": {"AssetPathName": "/Game/Portrait"},
                    "ActivityHeadIcon": {"AssetPathName": "/Game/Banner"},
                    "MaterialTexture": {"AssetPathName": "/Game/Banner"},
                    "OutlineColor": {"Hex": "ffcc00"}
                },
                "201": {
                    "ActivityHeadIcon": {"AssetPathName": "/Game/ForkBanner"},
                    "MaterialTexture": {"AssetPathName": "/Game/ForkMaterial"}
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
                    "ShowRewards": [
                        {"ItemID": "200", "IsUp": true}
                    ],
                    "BaseDropID": "drop_fork",
                    "UpGuaranteeCnt": 80,
                    "CurrencyID": "1010",
                    "CurrencyCnt": 1,
                    "OnceLotteryCnt": 1,
                    "Bg": {"AssetPathName": "/Game/UI/UI/Gacha/ForkBg.ForkBg"},
                    "Icon": {"AssetPathName": "/Game/UI/UI_Icon/Fork/1024/fork_Rose.fork_Rose"}
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
