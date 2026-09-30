#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_icons_use_default_appearance_heads_across_legacy_paths() {
        let tmp = tempfile::tempdir().unwrap();
        write_minimal_assets(tmp.path());
        let map = build_full_asset_map(tmp.path(), "zh-Hant").unwrap();

        for (id, icon) in [
            ("1004", "/Game/LacrimosaSquareHead"),
            ("1010", "/Game/SquareCharacterHead"),
            ("1036", "/Game/CharacterIcon"),
        ] {
            assert_eq!(map["items"][id]["asset_refs"]["icon"], json!(icon));
        }
    }

    #[test]
    fn character_and_fashion_icons_survive_cleared_show_times() {
        let tmp = tempfile::tempdir().unwrap();
        write_minimal_assets(tmp.path());
        let before = build_full_asset_map(tmp.path(), "zh-Hant").unwrap();

        let character_path = tmp.path().join(CHARACTER_TABLE);
        let mut characters = load_json(&character_path).unwrap();
        for id in ["1036", "1072"] {
            characters["Rows"][id]["ElementData"]["ShowTime"] = schedule(1, 1, 1, 0, 0, true);
        }
        write_json(character_path, characters);
        let after = build_full_asset_map(tmp.path(), "zh-Hant").unwrap();

        for id in [
            "1036",
            "Fashion_character_1036_03",
            "Fashion_character_1072_01",
        ] {
            assert_eq!(
                after["items"][id]["asset_refs"],
                before["items"][id]["asset_refs"]
            );
        }
    }

    #[test]
    fn builds_map_from_minimal_assets() {
        let tmp = tempfile::tempdir().unwrap();
        write_minimal_assets(tmp.path());

        let map = build_full_asset_map(tmp.path(), "zh-Hant").unwrap();

        let map_text = serde_json::to_string(&map).unwrap();
        assert!(!map_text.contains("\"source\""));
        assert!(!map_text.contains("\"domain_type\""));
        assert!(!map_text.contains("\"standard_5_pool\""));
        assert!(!map_text.contains("\"standard_4_pool\""));
        assert_eq!(map["items"]["1010"]["name"], "Character·Nanali");
        assert_eq!(map["items"]["1010"]["rarity"], 5);
        assert_eq!(map["items"]["201"]["name"], "Arc·Forgotten");
        assert_eq!(map["items"]["201"]["rarity"], 4);
        let nanali_refs = map["items"]["1010"]["asset_refs"].as_object().unwrap();
        assert_eq!(
            nanali_refs.get("head_icon"),
            Some(&json!("/Game/SmallIcon"))
        );
        assert_eq!(
            nanali_refs.get("icon"),
            Some(&json!("/Game/SquareCharacterHead"))
        );
        assert_eq!(
            nanali_refs.get("banner"),
            Some(&json!(
                "/Game/UI/UI/PlayerInfo/BusinessCards/Card_Small/YH_UI_bg_card_show_strip_08_s.YH_UI_bg_card_show_strip_08_s"
            ))
        );
        assert!(!nanali_refs.contains_key("portrait"));
        assert!(!nanali_refs.contains_key("material"));
        assert_eq!(
            map["banners"]["Lottery_Nanali"]["asset_refs"]["image"],
            "/Game/UI/UI/PlayerInfo/BusinessCards/Card_Small/YH_UI_bg_card_show_strip_08_s.YH_UI_bg_card_show_strip_08_s"
        );
        assert!(map["banners"]["Lottery_Nanali"]["asset_refs"]["featured_portraits"].is_null());
        let forgotten_refs = map["items"]["201"]["asset_refs"].as_object().unwrap();
        assert_eq!(forgotten_refs.get("icon"), Some(&json!("/Game/ForkIcon")));
        assert_eq!(
            forgotten_refs.get("head_icon"),
            Some(&json!("/Game/ForkSmallIcon"))
        );
        assert_eq!(
            forgotten_refs.get("banner"),
            Some(&json!("/Game/ForkBanner"))
        );
        assert_eq!(
            forgotten_refs.get("material"),
            Some(&json!("/Game/ForkMaterial"))
        );
        let glide_refs = map["items"]["Fashion_Glide_1010"]["asset_refs"]
            .as_object()
            .unwrap();
        assert_eq!(
            map["items"]["Fashion_Glide_1010"]["name"],
            "滑翔翼·Nanali Glide"
        );
        assert_eq!(map["items"]["Fashion_Glide_1010"]["category"], "glider");
        assert_eq!(glide_refs.get("icon"), Some(&json!("/Game/GlideIcon")));
        assert!(!glide_refs.contains_key("portrait"));
        assert!(!glide_refs.contains_key("banner"));
        assert_eq!(
            map["items"]["Fashion_character_1010"]["name"],
            "時裝·Nanali Fashion"
        );
        assert_eq!(
            map["items"]["Fashion_character_1010"]["category"],
            "fashion"
        );
        let fashion_refs = map["items"]["Fashion_character_1010"]["asset_refs"]
            .as_object()
            .unwrap();
        assert_eq!(
            fashion_refs.get("icon"),
            Some(&json!("/Game/LinkedFashionHead"))
        );
        assert_eq!(
            fashion_refs.get("head_icon"),
            Some(&json!("/Game/LinkedFashionIcon"))
        );
        assert_eq!(
            fashion_refs.get("portrait"),
            Some(&json!("/Game/LinkedFashionPortrait"))
        );
        assert_eq!(
            fashion_refs.get("banner"),
            Some(&json!("/Game/FashionBanner"))
        );
        let v13_distinct_refs = map["items"]["Fashion_character_1036_03"]["asset_refs"]
            .as_object()
            .unwrap();
        assert_eq!(
            v13_distinct_refs.get("icon"),
            Some(&json!("/Game/V13FashionIcon200"))
        );
        assert_eq!(
            v13_distinct_refs.get("head_icon"),
            Some(&json!("/Game/V13FashionIcon256"))
        );
        assert_eq!(
            v13_distinct_refs.get("portrait"),
            Some(&json!("/Game/V13FashionPortrait"))
        );
        let v13_missing_200_refs = map["items"]["Fashion_character_1072_01"]["asset_refs"]
            .as_object()
            .unwrap();
        assert_eq!(
            v13_missing_200_refs.get("icon"),
            Some(&json!("/Game/V13FashionRound256"))
        );
        assert!(!v13_missing_200_refs.contains_key("head_icon"));
        assert_eq!(
            v13_missing_200_refs.get("portrait"),
            Some(&json!("/Game/V13FashionPortrait1072"))
        );
        let character_refs = map["items"]["1036"]["asset_refs"].as_object().unwrap();
        assert_eq!(
            character_refs.get("icon"),
            Some(&json!("/Game/CharacterIcon"))
        );
        assert_eq!(
            character_refs.get("head_icon"),
            Some(&json!("/Game/CharacterHead"))
        );
        assert!(!character_refs.contains_key("portrait"));
        assert_eq!(map["labels"]["item_kind_fashion"], "時裝");
        assert_eq!(map["labels"]["item_kind_glider"], "滑翔翼");
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
        assert!(map["pools"]["ForkLottery_Test"]["asset_refs"].is_null());
        assert_eq!(
            map["banners"]["ForkLottery_Test"]["asset_refs"]["image"],
            "/Game/ForkUpIcon"
        );
        assert!(map["banners"]["ForkLottery_Test"]["asset_refs"]["background"].is_null());
        assert!(map["banners"]["ForkLottery_Test"]["asset_refs"]["icon"].is_null());
        assert_eq!(
            map["banners"]["Lottery_Nanali"]["end_at"],
            json!("2026-05-13 05:59:00")
        );
        assert_eq!(
            map["banners"]["Lottery_Nanali"]["mainland_end_at"],
            json!("2026-05-12 22:30:00")
        );
        assert!(map["banners"]["Lottery_Nanali"]["start_at"].is_null());
        assert!(map["banners"]["Lottery_Nanali"]["mainland_start_at"].is_null());
        assert_eq!(
            map["banners"]["Lottery_Nanali"]["rate_up_5"],
            json!(["1010"])
        );
        assert_eq!(
            map["banners"]["Lottery_Xun"]["start_at"],
            json!("2026-05-13 05:59:00")
        );
        assert_eq!(
            map["banners"]["Lottery_Xun"]["end_at"],
            json!("2026-06-03 05:59:00")
        );
        assert_eq!(map["banners"]["Lottery_Xun"]["rate_up_5"], json!(["1052"]));
        assert_eq!(
            map["banners"]["Lottery_AnHunQu"]["start_at"],
            json!("2026-06-03 05:59:00")
        );
        assert_eq!(
            map["banners"]["Lottery_AnHunQu"]["end_at"],
            json!("2026-06-24 05:59:00")
        );
        assert_eq!(
            map["banners"]["Lottery_AnHunQu"]["rate_up_5"],
            json!(["1004"])
        );
        assert_eq!(
            map["banners"]["Lottery_Kaesi"]["start_at"],
            json!("2026-06-24 05:59:00")
        );
        assert_eq!(
            map["banners"]["Lottery_Kaesi"]["end_at"],
            json!("2026-07-08 05:59:00")
        );
        assert_eq!(
            map["banners"]["Lottery_Kaesi"]["rate_up_5"],
            json!(["1071"])
        );
        assert_eq!(map["banners"]["Lottery_ZhenHong"]["title"], json!("破晓前"));
        assert_eq!(
            map["banners"]["Lottery_ZhenHong"]["start_at"],
            json!("2026-07-08 05:59:00")
        );
        assert_eq!(
            map["banners"]["Lottery_ZhenHong"]["end_at"],
            json!("2026-07-29 05:59:00")
        );
        assert_eq!(
            map["banners"]["Lottery_ZhenHong"]["rate_up_5"],
            json!(["1076"])
        );
        assert_eq!(map["banners"]["Lottery_Yiluoyi"]["title"], json!("生命线"));
        assert_eq!(
            map["banners"]["Lottery_Yiluoyi"]["start_at"],
            json!("2026-07-29 05:59:00")
        );
        assert_eq!(
            map["banners"]["Lottery_Yiluoyi"]["end_at"],
            json!("2026-08-19 05:59:00")
        );
        assert_eq!(
            map["banners"]["Lottery_Yiluoyi"]["rate_up_5"],
            json!(["1075"])
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

        let (core, overlay) = split_asset_map(&map, "zh-Hant").unwrap();
        assert_eq!(core["schema"], MAP_CORE_SCHEMA);
        assert_eq!(core["schema_version"], MAP_CORE_SCHEMA_VERSION);
        assert_eq!(overlay["schema"], MAP_LOCALE_SCHEMA);
        assert_eq!(overlay["schema_version"], MAP_LOCALE_SCHEMA_VERSION);
        assert_eq!(overlay["locale"], "zh-Hant");
        assert_eq!(overlay["items"]["1010"]["name"], "Character·Nanali");
        assert!(core["items"]["1010"].get("name").is_none());
        assert_eq!(
            overlay["banner_sources"]["Lottery_Nanali"]["title"],
            "Nanali Banner"
        );
        assert_eq!(
            core["banner_sources"]["Lottery_Nanali"]["source_id"],
            "Lottery_Nanali"
        );
        assert!(
            core["banner_sources"]["Lottery_Nanali"]
                .get("end_at")
                .is_none()
        );
    }

    #[test]
    fn localization_fallback_skips_whitespace_values() {
        let localization = BTreeMap::from([(
            "ST_Test".to_string(),
            BTreeMap::from([("name".to_string(), "   ".to_string())]),
        )]);
        let value = json!({
            "Key": "name",
            "TableId": "ST_Test",
            "SourceString": "Source fallback"
        });
        let layered_fallback = json!({
            "SourceString": " \t ",
            "LocalizedString": "Localized fallback"
        });

        assert_eq!(
            localized_text(Some(&value), &localization).as_deref(),
            Some("Source fallback")
        );
        assert_eq!(
            text_ref_source_text(Some(&layered_fallback), &localization).as_deref(),
            Some("Localized fallback")
        );
    }

    #[test]
    fn fills_only_exact_twenty_one_day_limited_boundary_gaps() {
        let boundary = |month, day| AssetDateTime {
            value: NaiveDate::from_ymd_opt(2026, month, day)
                .unwrap()
                .and_hms_opt(6, 30, 0)
                .unwrap(),
        };
        let mut exact = vec![Some(boundary(7, 8)), None, Some(boundary(8, 19))];
        let mut irregular = vec![Some(boundary(7, 8)), None, Some(boundary(8, 18))];

        fill_regular_limited_boundary_gaps(&mut exact);
        fill_regular_limited_boundary_gaps(&mut irregular);

        assert_eq!(exact[1], Some(boundary(7, 29)));
        assert_eq!(irregular[1], None);
    }

    #[test]
    fn rejects_inconsistent_limited_monopoly_order() {
        let tmp = tempfile::tempdir().unwrap();
        write_minimal_assets(tmp.path());
        write_json(
            tmp.path()
                .join("DataTable/Monopoly/DT_MonopolyCellDataTable.json"),
            json!({"Rows": {
                "1": {"PoolDropDatas": [
                    {"Key": "Lottery_Nanali"},
                    {"Key": "Lottery_Xun"},
                    {"Key": "Lottery_Permanent"},
                    {"Key": "Lottery_AnHunQu"},
                    {"Key": "Lottery_Kaesi"}
                ]},
                "2": {"PoolDropDatas": [
                    {"Key": "Lottery_Nanali"},
                    {"Key": "Lottery_AnHunQu"},
                    {"Key": "Lottery_Permanent"},
                    {"Key": "Lottery_Xun"},
                    {"Key": "Lottery_Kaesi"}
                ]}
            }}),
        );

        let error = build_full_asset_map(tmp.path(), "zh-Hant").unwrap_err();

        assert!(
            error
                .to_string()
                .contains("inconsistent monopoly limited pool order")
        );
    }

    #[test]
    fn rejects_case_folded_duplicate_item_ids() {
        let map = json!({
            "schema": MAP_CORE_SCHEMA,
            "schema_version": MAP_CORE_SCHEMA_VERSION,
            "items": {
                "Foo": {"rarity": 3},
                "foo": {"rarity": 4}
            },
            "item_aliases": {},
            "pools": {},
            "banner_sources": {},
            "gacha_rules": {}
        });

        let error = validate_core_source(&map, "test.json").unwrap_err();

        assert!(
            error
                .to_string()
                .contains("case-insensitive duplicate item_id")
        );
    }

    #[test]
    fn discovers_locales_and_skips_removed_locale() {
        let tmp = tempfile::tempdir().unwrap();
        write_json(tmp.path().join("DataTable/.keep"), json!({}));
        fs::create_dir_all(tmp.path().join("DataTable")).unwrap();
        for locale in ["en", "zh-Hant", "en-JM", "en-MT"] {
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
                    "item_type_8": "Fashion"
                },
                "ST_Ui": {
                    "BPUI_LotteryDiceRecord_biaozhunqipan": "Standard",
                    "BPUI_LotteryDiceRecord_xiandingqipan": "Limited",
                    "UW_LotteryBase_BP_Hupanyanmu": "Fork Group",
                    "Lottery_Kachimingcheng_changzhu": "Standard Title",
                    "Lottery_Kachimingcheng_nanali": "Nanali Banner",
                    "LotteryDes_Jishishuoming_NanaliDes": "1.<Orange>\"Nanali Banner\"</> is a limited board.",
                    "LotteryDes_Jishishuoming_XunDes": "1.<Orange>\"Xun Banner\"</> is a limited board.",
                    "LotteryDes_Jishishuoming_AnHunQuDes": "1.<Orange>\"Waking Reverie\"</> is a limited board.",
                    "LotteryDes_Jishishuoming_KaesiDes": "1.<Orange>\"Kaesi Banner\"</> is a limited board.",
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
            root.join("Localization/zh-CN/game.json"),
            json!({
                "ST_Ui": {
                    "LotteryDes_Jishishuoming_ZhenhongDes": "1.<Orange>「破晓前」</>属于<Orange>「限定棋盘」</>。",
                    "LotteryDes_Jishishuoming_YiluoyiDes": "1.<Orange>「生命线」</>属于<Orange>「限定棋盘」</>。",
                    "Lottery_kachimingcheng_zhenhong": "破晓前",
                    "Lottery_Kachimingcheng_yiluoyi": "生命线"
                }
            }),
        );
        write_json(
            root.join("Localization/zh-Hant/game.json"),
            json!({
                "ST_Common": {
                    "item_type_3": "Character",
                    "item_type_5": "Arc",
                    "item_type_8": "時裝",
                    "item_type_10": "Mod Parts"
                },
                "ST_Ui": {
                    "BPUI_LotteryDiceRecord_biaozhunqipan": "標準棋盤",
                    "BPUI_LotteryDiceRecord_xiandingqipan": "限定棋盤",
                    "UW_LotteryBase_BP_Hupanyanmu": "Fork Group",
                    "Lottery_Kachimingcheng_changzhu": "Standard Title",
                    "Lottery_Kachimingcheng_nanali": "Nanali Banner",
                    "LotteryDes_Jishishuoming_NanaliDes": "1.<Orange>「Nanali Banner」</>屬於<Orange>「限定棋盤」</>。",
                    "LotteryDes_Jishishuoming_XunDes": "1.<Orange>「Xun Banner」</>屬於<Orange>「限定棋盤」</>。",
                    "LotteryDes_Jishishuoming_AnHunQuDes": "1.<Orange>「久夢初醒時」</>屬於<Orange>「限定棋盤」</>。",
                    "LotteryDes_Jishishuoming_KaesiDes": "1.<Orange>「Kaesi Banner」</>屬於<Orange>「限定棋盤」</>。",
                    "ui_appearance_02": "滑翔翼"
                },
                "ST_Item": {
                    "char_1004": "Lacrimosa",
                    "char_1010": "Nanali",
                    "char_1052": "Xun",
                    "char_1071": "Kaesi",
                    "char_1075": "Yiluoyi",
                    "char_1076": "ZhenHong",
                    "char_1036": "Canhong",
                    "char_1072": "Linko",
                    "Dicelimite_lacrimosa_usedesc": "僅可在限定棋盤「久夢初醒時」中進行投擲獲得所需物品。",
                    "fork_200": "Fork Weapon",
                    "fork_201": "Forgotten",
                    "fashion_1010": "Nanali Fashion",
                    "fashion_1036_v13": "Canhong V1.3 Fashion",
                    "fashion_1072_v13": "Linko V1.3 Fashion",
                    "glide_1010": "Nanali Glide",
                    "vehicle_1010": "Nanali Vehicle",
                    "vehicle_skin_1010": "Nanali Vehicle Skin"
                }
            }),
        );
        write_json(
            root.join("DataTable/Character/DT_Character.json"),
            json!({"Rows": {
                "1004": character_row("char_1004", "/Game/LacrimosaPortrait", Value::Null),
                "1010": {
                    "ItemName": {"Key": "char_1010", "TableId": "ST_Item"},
                    "ItemQuality": "EItemQuality::ITEM_QUALITY_ORANGE",
                    "ItemIcon": {"AssetPathName": "/Game/Portrait"},
                    "ItemIconSmall": {"AssetPathName": "/Game/SmallIcon"},
                    "ItemIconBig": {"AssetPathName": "/Game/Portrait"},
                    "ElementData": {"ShowTime": schedule(1, 1, 1, 0, 0, true)}
                },
                "1052": character_row(
                    "char_1052",
                    "/Game/XunPortrait",
                    schedule(2026, 5, 12, 22, 30, true)
                ),
                "1071": character_row(
                    "char_1071",
                    "/Game/KaesiPortrait",
                    schedule(2026, 6, 23, 22, 30, true)
                ),
                "1075": character_row(
                    "char_1075",
                    "/Game/YiluoyiPortrait",
                    schedule(2026, 7, 28, 22, 30, true)
                ),
                "1076": character_row(
                    "char_1076",
                    "/Game/ZhenHongPortrait",
                    schedule(2026, 7, 7, 22, 30, true)
                ),
                "1036": {
                    "ItemName": {"Key": "char_1036", "TableId": "ST_Item"},
                    "ItemQuality": "EItemQuality::ITEM_QUALITY_ORANGE",
                    "ItemIcon": {"AssetPathName": "/Game/CharacterIcon"},
                    "ItemIconSmall": {"AssetPathName": "/Game/CharacterHead"},
                    "ItemIconBig": {"AssetPathName": "/Game/CharacterBig"},
                    "ToolDatas": [{"AppearanceID": "Fashion_1036_1"}],
                    "ElementData": {"ShowTime": schedule(2026, 8, 13, 6, 30, true)}
                },
                "1072": {
                    "ItemName": {"Key": "char_1072", "TableId": "ST_Item"},
                    "ItemQuality": "EItemQuality::ITEM_QUALITY_ORANGE",
                    "ElementData": {"ShowTime": schedule(2026, 9, 3, 6, 30, true)}
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
                "1010_LotteryShow_nanali": lottery_show_row("1010_LotteryShow_nanali", "Nanali（1.8700%）"),
                "1052_LotteryShow_xun": lottery_show_row("1052_LotteryShow_xun", "Xun（1.8700%）"),
                "1004_LotteryShow_lacrimosa": lottery_show_row("1004_LotteryShow_lacrimosa", "Lacrimosa（1.8700%）"),
                "1071_LotteryShow_kaesi": lottery_show_row("1071_LotteryShow_kaesi", "Kaesi（1.8700%）"),
                "1075_LotteryShow_yiluoyi": lottery_show_row("1075_LotteryShow_yiluoyi", "伊洛伊（1.8700%）"),
                "1076_LotteryShow_shinku": lottery_show_row("1076_LotteryShow_shinku", "真红（1.8700%）"),
                "Dicelimite_lacrimosa": {
                    "ItemName": {"Key": "dice_lacrimosa", "TableId": "ST_Item"},
                    "UseContext": {
                        "Key": "Dicelimite_lacrimosa_usedesc",
                        "TableId": "ST_Item",
                        "SourceString": "僅可在限定棋盤「久夢初醒時」中進行投擲獲得所需物品。"
                    },
                    "ItemQuality": "EItemQuality::ITEM_QUALITY_ORANGE"
                },
                "Dicelimite_onerioi": {
                    "ItemName": {"Key": "dice_onerioi", "TableId": "ST_Item"},
                    "UseContext": {
                        "Key": "Dicelimite_onerioi_usedesc",
                        "TableId": "ST_Item",
                        "SourceString": "仅可在限定棋盘「生命线」中进行投掷获得所需物品。"
                    },
                    "ItemQuality": "EItemQuality::ITEM_QUALITY_ORANGE"
                },
                "Dicelimite_shinku": {
                    "ItemName": {"Key": "dice_shinku", "TableId": "ST_Item"},
                    "UseContext": {
                        "Key": "Dicelimite_shinku_uesdesc",
                        "TableId": "ST_Item",
                        "SourceString": "仅可在限定棋盘「破晓前」中进行投掷获得所需物品。"
                    },
                    "ItemQuality": "EItemQuality::ITEM_QUALITY_ORANGE"
                },
                "Fashion_vehicle_1010_V008": {
                    "ItemName": {"Key": "vehicle_1010", "TableId": "ST_Item"},
                    "ItemQuality": "EItemQuality::ITEM_QUALITY_ORANGE"
                },
                "Fashion_character_1010": {
                    "ItemName": {"Key": "fashion_1010", "TableId": "ST_Item"},
                    "ItemQuality": "EItemQuality::ITEM_QUALITY_ORANGE",
                    "ItemIcon": {"AssetPathName": "/Game/WrongFashionIcon"},
                    "ItemIconSmall": {"AssetPathName": "/Game/WrongFashionHead"},
                    "ToolDatas": [{"AppearanceID": "Fashion_1010_2"}]
                },
                "Fashion_character_1036_03": {
                    "ItemName": {"Key": "fashion_1036_v13", "TableId": "ST_Item"},
                    "ItemQuality": "EItemQuality::ITEM_QUALITY_ORANGE",
                    "ToolDatas": [{"AppearanceID": "Fashion_1036_3"}]
                },
                "Fashion_character_1072_01": {
                    "ItemName": {"Key": "fashion_1072_v13", "TableId": "ST_Item"},
                    "ItemQuality": "EItemQuality::ITEM_QUALITY_ORANGE",
                    "ToolDatas": [{"AppearanceID": "Fashion_1072_space"}]
                },
                "Fashion_vehicleSkin_1010_V001": {
                    "ItemName": {"Key": "vehicle_skin_1010", "TableId": "ST_Item"},
                    "ItemQuality": "EItemQuality::ITEM_QUALITY_ORANGE"
                }
            }}),
        );
        write_json(
            root.join("DataTable/Monopoly/DT_MonopolyCellDataTable.json"),
            json!({
                "Rows": {
                    "1": {"PoolDropDatas": [
                        {"Key": "Lottery_Nanali"},
                        {"Key": "Lottery_Xun"},
                        {"Key": "Lottery_Permanent"},
                        {"Key": "Lottery_AnHunQu"},
                        {"Key": "Lottery_Kaesi"},
                        {"Key": "Lottery_ZhenHong", "Value": {"MapDropDatas": [{"Value": "ZhenHong_Chess_LessMiDie"}]}},
                        {"Key": "Lottery_Yiluoyi", "Value": {"MapDropDatas": [{"Value": "Yiluoyi_Chess_LessMiDie"}]}}
                    ]},
                    "2": {"PoolDropDatas": [
                        {"Key": "Lottery_Nanali"},
                        {"Key": "Lottery_Xun"},
                        {"Key": "Lottery_Permanent"},
                        {"Key": "Lottery_AnHunQu"},
                        {"Key": "Lottery_Kaesi"},
                        {"Key": "Lottery_ZhenHong", "Value": {"MapDropDatas": [{"Value": "ZhenHong_Character_ZhenHong"}]}},
                        {"Key": "Lottery_Yiluoyi", "Value": {"MapDropDatas": [{"Value": "Yiluoyi_Character_Yiluoyi"}]}}
                    ]}
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
            root.join("DataTable/Character/Appearance/DT_AppearanceData.json"),
            json!({"Rows": {
                "base_lacrimosa": {
                    "AppearanceType": "EAppearanceType::Fashion",
                    "CharacterID": "1004",
                    "IsDefault": true,
                    "HeadIcon": {"AssetPathName": "/Game/LacrimosaSquareHead"},
                    "HeadIconBig": {"AssetPathName": "/Game/LacrimosaPortrait"}
                },
                "base_nanali": {
                    "AppearanceType": "EAppearanceType::Fashion",
                    "CharacterID": "1010",
                    "IsDefault": true,
                    "HeadIcon": {"AssetPathName": "/Game/SquareCharacterHead"},
                    "HeadIconBig": {"AssetPathName": "/Game/LegacyCharacterIcon256"}
                },
                "Fashion_Glide_1010": {
                    "Name": {"Key": "glide_1010", "TableId": "ST_Item"},
                    "Quality": "EItemQuality::ITEM_QUALITY_ORANGE",
                    "AppearanceType": "EAppearanceType::Glide",
                    "DisplayIcon": {"AssetPathName": "/Game/GlideIcon"},
                    "HeadIconBig": {"AssetPathName": "/Game/GlidePortrait"},
                    "PortraitImg": {"AssetPathName": "/Game/GlideBanner"}
                },
                "Fashion_1010_2": {
                    "AppearanceType": "EAppearanceType::Fashion",
                    "CharacterID": "1010",
                    "HeadIconBig": {"AssetPathName": "/Game/LinkedFashionIcon"},
                    "HeadIcon": {"AssetPathName": "/Game/LinkedFashionHead"},
                    "SmallPortraitImg": {"AssetPathName": "/Game/LinkedFashionPortrait"}
                },
                "Fashion_1036_1": {
                    "AppearanceType": "EAppearanceType::Fashion",
                    "CharacterID": "1036",
                    "IsDefault": true,
                    "HeadIconBig": {"AssetPathName": "/Game/LinkedCharacterIcon"},
                    "HeadIcon": {"AssetPathName": "/Game/CharacterIcon"},
                    "SmallPortraitImg": {"AssetPathName": "/Game/LinkedCharacterPortrait"}
                },
                "Fashion_1036_3": {
                    "AppearanceType": "EAppearanceType::Fashion",
                    "CharacterID": "1036",
                    "HeadIconBig": {"AssetPathName": "/Game/V13FashionIcon256"},
                    "HeadIcon": {"AssetPathName": "/Game/V13FashionIcon200"},
                    "DisplayIcon": {"AssetPathName": "/Game/V13FashionDisplay1036"},
                    "SmallPortraitImg": {"AssetPathName": "/Game/V13FashionPortrait"}
                },
                "Fashion_1072_space": {
                    "AppearanceType": "EAppearanceType::Fashion",
                    "CharacterID": "1072",
                    "HeadIconBig": {"AssetPathName": "/Game/V13FashionRound256"},
                    "HeadIcon": {"AssetPathName": "/Game/V13FashionRound256"},
                    "DisplayIcon": {"AssetPathName": "/Game/V13FashionDisplay"},
                    "SmallPortraitImg": {"AssetPathName": "/Game/V13FashionPortrait1072"}
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
                    "HeadIcon": {"AssetPathName": "/Game/LegacyCharacterIcon256"},
                    "ItemIcon": {"AssetPathName": "/Game/Portrait"},
                    "ActivityHeadIcon": {"AssetPathName": "/Game/Banner"},
                    "MaterialTexture": {"AssetPathName": "/Game/Banner"},
                    "OutlineColor": {"Hex": "ffcc00"}
                },
                "201": {
                    "ActivityHeadIcon": {"AssetPathName": "/Game/ForkBanner"},
                    "MaterialTexture": {"AssetPathName": "/Game/ForkMaterial"}
                },
                "Fashion_character_1010": {
                    "HeadIcon": {"AssetPathName": "/Game/WrongGachaFashionIcon"},
                    "ItemIcon": {"AssetPathName": "/Game/WrongGachaFashionPortrait"},
                    "ActivityHeadIcon": {"AssetPathName": "/Game/FashionBanner"}
                },
                "Fashion_character_1036_03": {
                    "HeadIcon": {"AssetPathName": "/Game/WrongV13GachaIcon"},
                    "ItemIcon": {"AssetPathName": "/Game/WrongV13GachaPortrait"}
                },
                "Fashion_character_1072_01": {
                    "HeadIcon": {"AssetPathName": "/Game/WrongV13GachaIcon1072"},
                    "ItemIcon": {"AssetPathName": "/Game/WrongV13GachaPortrait1072"}
                },
                "1036": {
                    "HeadIcon": {"AssetPathName": "/Game/WrongGachaCharacterIcon"},
                    "ItemIcon": {"AssetPathName": "/Game/WrongGachaCharacterPortrait"}
                },
                "Fashion_Glide_1010": {},
                "Fashion_vehicle_1010_V008": {},
                "Fashion_vehicleSkin_1010_V001": {}
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
        write_json(
            root.join("DataTable/CombatAward/DT_CombatAwardEntranceConfig.json"),
            json!({"Rows": {
                "CombatAward_02": {"StartDateTime": schedule(2026, 4, 28, 22, 30, true)},
                "CombatAward_03": {"StartDateTime": schedule(2026, 6, 2, 22, 30, true)},
                "CombatAward_04": {"StartDateTime": schedule(2026, 7, 7, 22, 30, true)}
            }}),
        );
    }

    fn character_row(name_key: &str, portrait: &str, show_time: Value) -> Value {
        json!({
            "ItemName": {"Key": name_key, "TableId": "ST_Item"},
            "ItemQuality": "EItemQuality::ITEM_QUALITY_ORANGE",
            "ItemIcon": {"AssetPathName": portrait},
            "ItemIconSmall": {"AssetPathName": portrait},
            "ItemIconBig": {"AssetPathName": portrait},
            "ElementData": {"ShowTime": show_time}
        })
    }

    fn lottery_show_row(name_key: &str, source: &str) -> Value {
        json!({
            "ItemName": {
                "Key": name_key,
                "TableId": "ST_Ui",
                "SourceString": source
            },
            "ItemQuality": "EItemQuality::ITEM_QUALITY_ORANGE"
        })
    }

    fn schedule(year: u64, month: u64, day: u64, hour: u64, minute: u64, utc: bool) -> Value {
        json!({
            "MainlandTime": {
                "Year": year,
                "Month": month,
                "Day": day,
                "Hour": hour,
                "minute": minute,
                "Second": 0
            },
            "OverseaTime": {
                "Year": year,
                "Month": month,
                "Day": day,
                "Hour": hour,
                "minute": minute,
                "Second": 0
            },
            "OverseaUseUTCTime": utc
        })
    }

    fn write_json(path: PathBuf, value: Value) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    }
}
