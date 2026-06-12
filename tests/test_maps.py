from __future__ import annotations

import json

from nte_gacha_exporter.mapping.assets import ITEM_ID_SOURCE_PRIORITY, _known_item_id_priorities, build_map
from nte_gacha_exporter.mapping.runtime import available_locales, load_map


def test_all_maps_have_public_schema_only():
    for locale in available_locales():
        data = load_map(locale)
        assert set(data) == {"csv_headers", "items", "pools", "pool_meta", "labels"}


def test_zh_hant_key_mappings():
    data = load_map("zh-Hant")
    assert data["items"]["Fashion_vehicle_1010_V008"] == "改裝件·萌虎來襲-塗裝"
    assert data["items"]["Fashion_vehicle_1052_V024"] == "改裝件·秋色殘影-塗裝"
    assert data["items"]["Fashion_character_1004_01"] == "時裝·鎏金交響詩"
    assert data["items"]["Characterawaken_dafudier"] == "道具·心象碎片·達芙蒂爾"
    assert data["items"]["DiceNormal"] == "道具·捏造骰子"
    assert data["items"]["DIceNormal"] == "道具·捏造骰子"
    assert "Annulith" not in data["items"]
    assert "Vehicle026" not in data["items"]
    assert "Fashion_character_1051_07" not in data["items"]
    assert "fork_jianang" not in data["items"]
    assert data["pools"]["CardPool_NewRole"] == "標準棋盤"
    assert data["pools"]["CardPool_Character"] == "限定棋盤"
    assert data["pools"]["1"] == "奇蹟盒盒"
    assert data["pool_meta"]["CardPool_NewRole"] == {
        "group_label": "標準棋盤",
        "title": "世間奇遇",
    }
    assert data["pool_meta"]["CardPool_Character"] == {
        "group_label": "限定棋盤",
        "title_windows": [
            {"end_at_tz8": "2026-05-13 05:59:00", "title": "王牌一代目"},
            {"end_at_tz8": "2026-06-03 05:59:00", "title": "獨酌朧月流"},
            {"end_at_tz8": "2026-06-24 05:59:00", "title": "久夢初醒時"},
            {"end_at_tz8": "2026-07-08 05:59:00", "title": "無歸路"},
        ],
    }
    assert data["pool_meta"]["ForkLottery_AnHunQu"] == {
        "group_label": "弧盤研募",
        "title": "夜曲特刊",
        "subtitle": "限定弧盤「最後一朵玫瑰」機率提升!",
    }


def test_build_map_uses_gacha_refs_and_semantic_names(tmp_path):
    def write_json(relative: str, data: object) -> None:
        path = tmp_path / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(data, ensure_ascii=False), encoding="utf-8")

    write_json(
        "Localization/zh-Hant/game.json",
        {
            "ST_Appearance": {"Fashion_1004_1_Name": "鎏金交響詩", "Glide_2_3_Name": "好柿成雙"},
            "ST_Common": {
                "item_type_2": "道具",
                "item_type_5": "弧盤",
                "item_type_8": "時裝",
                "item_type_9": "滑翔翼",
                "item_type_10": "改裝件",
                "item_type_99": "活動票券",
            },
            "ST_Item": {
                "Characterawaken_dafudier_name": "心象碎片·達芙蒂爾",
                "DiceNormal_Name": "捏造骰子",
                "EventTokenA_name": "測試票券",
                "fork_dustbin_name": "危險遊戲",
                "fork_jianang_name": "佳釀",
                "Fashion_glide_1004_name": "錯誤通用名",
                "item_Annulith_name": "環石",
            },
            "ST_Ui": {
                "Abyss_GamepadKeys_1": "切換",
                "AbyssClone_Award_02": "已完成",
                "BPUI_GashaponRecord_time": "獲得時間",
                "BPUI_LotteryDiceRecord_biaozhunqipan": "標準棋盤",
                "BPUI_LotteryDiceRecord_daojumingcheng": "道具名稱",
                "BPUI_LotteryDiceRecord_qipanleixing": "棋盤類型",
                "BPUI_LotteryDiceRecord_touzhidianshu": "投擲點數",
                "BPUI_LotteryDiceRecord_xiandingqipan": "限定棋盤",
                "BPUI_LotteryModuleEntrance_Title": "斯卡布羅集市",
                "BPUI_LotteryResult_AdditionalReward": "額外獲得",
                "LotteryDes_Jishishuoming_AnHunQuDes": "1.<Orange>「久夢初醒時」</>屬於<Orange>「限定棋盤」</>。",
                "LotteryDes_Jishishuoming_KaesiDes": "1.<Orange>「無歸路」</>屬於<Orange>「限定棋盤」</>。",
                "LotteryDes_Jishishuoming_NanaliDes": "1.<Orange>「王牌一代目」</>屬於<Orange>「限定棋盤」</>。",
                "LotteryDes_Jishishuoming_XunDes": "1.<Orange>「獨酌朧月流」</>屬於<Orange>「限定棋盤」</>。",
                "Lottery_Kachimingcheng_changzhu": "世間奇遇",
                "Mall_8_name": "弧盤商店",
                "TreasureBox_2": "開啟",
                "UI_Lottery_GachaDetails_Zhitoujilu": "擲骰記錄",
                "UI_Lottery_GachaDetails_title": "棋盤詳情",
                "UI_CloneSystemChallengeFailed_Retry": "重試",
                "UI_CloneSystemStaminaTips_Enter": "進入",
                "ui_appearance_02": "滑翔翼",
                "ui_forkshop_24": "奇蹟盒盒",
                "UW_LotteryBase_BP_Hupanyanmu": "弧盤研募",
                "W_HTButton_Next_Page": "下一頁",
                "W_Vehicle_Button_Choose": "選擇",
                "ui_forklottery_up_anhunqu_01": "夜曲特刊",
                "ui_forklottery_up_anhunqu_02": "限定弧盤「最後一朵玫瑰」機率提升!",
                "ui_forkshop_07": "研募詳情",
                "ui_forkshop_10": "研募記錄",
            },
            "ST_UI_C": {"BPUI_CharacterEquipDevFilter_15": "類型"},
            "ST_UI_hpy": {"MangHe_09": "數量"},
            "ST_VehicleData": {"V008_Decal_07_name": "萌虎來襲-塗裝"},
        },
    )
    write_json(
        "DataTable/Inventory/DT_ItemType.json",
        [
            {
                "Rows": {
                    "ITEM_TYPE_FASHION": {
                        "TypeName": {"TableId": "/Game/Text/ST_Common.ST_Common", "Key": "item_type_8"}
                    },
                    "ITEM_TYPE_VEHICLE_SKIN": {
                        "TypeName": {"TableId": "/Game/Text/ST_Common.ST_Common", "Key": "item_type_10"}
                    },
                    "ITEM_TYPE_GLIDE": {
                        "TypeName": {"TableId": "/Game/Text/ST_Common.ST_Common", "Key": "item_type_9"}
                    },
                    "ITEM_TYPE_EVENT_TOKEN": {
                        "TypeName": {"TableId": "/Game/Text/ST_Common.ST_Common", "Key": "item_type_99"}
                    },
                }
            }
        ],
    )
    write_json(
        "DataTable/Gacha/GachaIllustrate.json",
        [
            {
                "Rows": {
                    "Characterawaken_dafudier": {},
                    "DIceNormal": {},
                    "EventTokenA": {},
                    "Fashion_glide_1004": {},
                }
            }
        ],
    )
    write_json(
        "DataTable/Inventory/DT_ItemConfig.json",
        [
            {
                "Rows": {
                    "Annulith": {"ItemName": {"TableId": "/Game/Text/ST_Item.ST_Item", "Key": "item_Annulith_name"}},
                    "DiceNormal": {
                        "ItemType": "EItemType::ITEM_TYPE_Lottery",
                        "ItemName": {"TableId": "/Game/Text/ST_Item.ST_Item", "Key": "DiceNormal_Name"},
                    },
                    "Fashion_character_1004_01": {
                        "ItemType": "EItemType::ITEM_TYPE_FASHION",
                        "ItemName": {"TableId": "/Game/Text/ST_Appearance.ST_Appearance", "Key": "Fashion_1004_1_Name"},
                    },
                    "Fashion_glide_1004": {
                        "ItemType": "EItemType::ITEM_TYPE_GLIDE",
                        "ItemName": {"TableId": "/Game/Text/ST_Item.ST_Item", "Key": "Fashion_glide_1004_name"},
                    },
                    "EventTokenA": {
                        "ItemType": "EItemType::ITEM_TYPE_EVENT_TOKEN",
                        "ItemName": {"TableId": "/Game/Text/ST_Item.ST_Item", "Key": "EventTokenA_name"},
                    },
                }
            }
        ],
    )
    write_json(
        "DataTable/Gacha/DT_LotteryDataTable_NewPool.json",
        [
            {
                "Rows": {
                    "NewRow": {
                        "SSRItems": [{"ItemID": "fork_jianang", "bIsShowIconTip": False}],
                    }
                }
            }
        ],
    )
    write_json(
        "DataTable/Character/Appearance/DT_AppearanceData.json",
        [
            {
                "Rows": {
                    "Fashion_Glide_1004": {
                        "AppearanceType": "EAppearanceType::Glide",
                        "Name": {"TableId": "/Game/Text/ST_Appearance.ST_Appearance", "Key": "Glide_2_3_Name"},
                    }
                }
            }
        ],
    )
    write_json(
        "DataTable/Fork/DT_ForkItemData.json",
        [
            {
                "Rows": {
                    "fork_dustbin": {"ItemName": {"TableId": "/Game/Text/ST_Item.ST_Item", "Key": "fork_dustbin_name"}},
                    "fork_jianang": {"ItemName": {"TableId": "/Game/Text/ST_Item.ST_Item", "Key": "fork_jianang_name"}},
                }
            }
        ],
    )
    write_json(
        "DataTable/Drop/Client/ClientDropGroupDataTable.json",
        [
            {
                "Rows": {
                    "drop_ForkPull_AnHunQu_0": {"SequenceId": "droplist_ForkPull_gold_AnHunQu"},
                    "drop_Monopoly_fashion_character_lacrimosa_0": {"SequenceId": "droplist_Fashion_character_1004"},
                    "drop_Monopoly_fashion_vehicle_nanally_0": {"SequenceId": "droplist_Fashion_vehicle_1010"},
                }
            }
        ],
    )
    write_json(
        "DataTable/Drop/DropSequenceDataTable.json",
        [
            {
                "Rows": {
                    "droplist_ForkPull_gold_AnHunQu_0": {"ItemID": "fork_dustbin"},
                    "droplist_Fashion_character_1004_0": {"ItemID": "Fashion_character_1004_01"},
                    "droplist_Fashion_vehicle_1010_0": {"ItemID": "Fashion_vehicle_1010_V008"},
                }
            }
        ],
    )
    write_json(
        "DataTable/Vehicle/DT_vehicleModuleData.json",
        [
            {
                "Rows": {
                    "V008_Decal_08": {
                        "ModuleName": {
                            "TableId": "/Game/Text/ST_VehicleData.ST_VehicleData",
                            "Key": "V008_Decal_07_name",
                        },
                        "FeatureActiveData": {"Requires": [{"ID": "Fashion_vehicle_1010_V008"}]},
                    }
                }
            }
        ],
    )
    write_json(
        "DataTable/Fork/DT_ForkLotteryPoolData.json",
        [
            {
                "Rows": {
                    "1": {"Name": {"TableId": "/Game/Text/ST_Ui.ST_Ui", "Key": "ui_forkshop_24"}},
                    "ForkLottery_AnHunQu": {
                        "Name": {"TableId": "/Game/Text/ST_Ui.ST_Ui", "Key": "ui_forkshop_24"},
                        "ShowText1": {"TableId": "/Game/Text/ST_Ui.ST_Ui", "Key": "ui_forklottery_up_anhunqu_01"},
                        "ShowText2": {"TableId": "/Game/Text/ST_Ui.ST_Ui", "Key": "ui_forklottery_up_anhunqu_02"},
                        "ShowRewards": [{"ItemID": "fork_dustbin"}],
                        "ExtraRewards": [{"Value": {"ItemList": [{"ItemID": "DiceNormal"}]}}],
                        "BaseDropID": "drop_ForkPull_AnHunQu",
                    },
                }
            }
        ],
    )

    data = build_map(tmp_path, "zh-Hant")

    assert set(data) == {"csv_headers", "items", "pools", "pool_meta", "labels"}
    assert data["csv_headers"] == {
        "count": "數量",
        "item_name": "道具名稱",
        "pool_group": "卡池類型",
        "pool_name": "卡池",
        "roll_label": "投擲點數",
        "secondary_count": "額外獲得數量",
        "secondary_item_name": "額外獲得",
        "time": "獲得時間",
    }
    assert data["items"]["Characterawaken_dafudier"] == "道具·心象碎片·達芙蒂爾"
    assert data["items"]["DIceNormal"] == "道具·捏造骰子"
    assert data["items"]["DiceNormal"] == "道具·捏造骰子"
    assert data["items"]["EventTokenA"] == "活動票券·測試票券"
    assert data["items"]["Fashion_character_1004_01"] == "時裝·鎏金交響詩"
    assert data["items"]["Fashion_Glide_1004"] == "滑翔翼·好柿成雙"
    assert data["items"]["Fashion_glide_1004"] == "滑翔翼·好柿成雙"
    assert data["items"]["Fashion_vehicle_1010_V008"] == "改裝件·萌虎來襲-塗裝"
    assert data["items"]["fork_dustbin"] == "弧盤·危險遊戲"
    assert data["items"]["fork_jianang"] == "弧盤·佳釀"
    assert "Annulith" not in data["items"]
    assert "fork_missing" not in data["items"]
    assert data["pools"]["CardPool_NewRole"] == "標準棋盤"
    assert data["pools"]["CardPool_Character"] == "限定棋盤"
    assert data["pools"]["1"] == "奇蹟盒盒"
    assert data["pool_meta"]["CardPool_NewRole"] == {
        "group_label": "標準棋盤",
        "title": "世間奇遇",
    }
    assert data["pool_meta"]["CardPool_Character"] == {
        "group_label": "限定棋盤",
        "title_windows": [
            {"end_at_tz8": "2026-05-13 05:59:00", "title": "王牌一代目"},
            {"end_at_tz8": "2026-06-03 05:59:00", "title": "獨酌朧月流"},
            {"end_at_tz8": "2026-06-24 05:59:00", "title": "久夢初醒時"},
            {"end_at_tz8": "2026-07-08 05:59:00", "title": "無歸路"},
        ],
    }
    assert data["pool_meta"]["ForkLottery_AnHunQu"] == {
        "group_label": "弧盤研募",
        "title": "夜曲特刊",
        "subtitle": "限定弧盤「最後一朵玫瑰」機率提升!",
    }
    assert data["labels"]["Abyss_GamepadKeys_1"] == "切換"
    assert data["labels"]["AbyssClone_Award_02"] == "已完成"
    assert data["labels"]["BPUI_LotteryDiceRecord_biaozhunqipan"] == "標準棋盤"
    assert data["labels"]["BPUI_LotteryDiceRecord_qipanleixing"] == "棋盤類型"
    assert data["labels"]["BPUI_LotteryDiceRecord_xiandingqipan"] == "限定棋盤"
    assert data["labels"]["BPUI_LotteryModuleEntrance_Title"] == "斯卡布羅集市"
    assert data["labels"]["Mall_8_name"] == "弧盤商店"
    assert data["labels"]["TreasureBox_2"] == "開啟"
    assert data["labels"]["UI_CloneSystemChallengeFailed_Retry"] == "重試"
    assert data["labels"]["UI_CloneSystemStaminaTips_Enter"] == "進入"
    assert data["labels"]["UI_Lottery_GachaDetails_Zhitoujilu"] == "擲骰記錄"
    assert data["labels"]["UI_Lottery_GachaDetails_title"] == "棋盤詳情"
    assert data["labels"]["UW_LotteryBase_BP_Hupanyanmu"] == "弧盤研募"
    assert data["labels"]["W_HTButton_Next_Page"] == "下一頁"
    assert data["labels"]["W_Vehicle_Button_Choose"] == "選擇"
    assert data["labels"]["ui_forkshop_07"] == "研募詳情"
    assert data["labels"]["ui_forkshop_10"] == "研募記錄"


def test_build_map_uses_english_asset_fallback_locale(tmp_path):
    def write_json(relative: str, data: object) -> None:
        path = tmp_path / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(data, ensure_ascii=False), encoding="utf-8")

    write_json(
        "Localization/en/game.json",
        {
            "ST_Item": {"DiceNormal_Name": "Test Dice"},
        },
    )
    write_json(
        "Localization/zh-Hant/game.json",
        {
            "ST_Common": {"item_type_2": "道具"},
            "ST_Item": {"DiceNormal_Name": "中文骰子"},
        },
    )
    write_json("DataTable/Gacha/GachaIllustrate.json", [{"Rows": {"DiceNormal": {}}}])
    write_json(
        "DataTable/Inventory/DT_ItemConfig.json",
        [
            {
                "Rows": {
                    "DiceNormal": {
                        "ItemType": "EItemType::ITEM_TYPE_Lottery",
                        "ItemName": {"TableId": "/Game/Text/ST_Item.ST_Item", "Key": "DiceNormal_Name"},
                    },
                }
            }
        ],
    )

    data = build_map(tmp_path, "missing-locale")

    assert data["items"]["DiceNormal"] == "Item·Test Dice"


def test_item_id_priorities_use_each_source_kind(tmp_path):
    def write_rows(relative: str, rows: dict[str, object]) -> None:
        path = tmp_path / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps([{"Rows": rows}], ensure_ascii=False), encoding="utf-8")

    write_rows("DataTable/Vehicle/DT_VehicleData.json", {"VehicleCase": {}})
    write_rows("DataTable/Character/Appearance/DT_AppearanceData.json", {"AppearanceCase": {}})
    write_rows(
        "DataTable/Vehicle/DT_vehicleModuleData.json",
        {"ModuleCase": {"FeatureActiveData": {"Requires": [{"ID": "VehicleModuleCase"}]}}},
    )

    priorities = _known_item_id_priorities(tmp_path, {})

    assert priorities["VehicleCase"] == ITEM_ID_SOURCE_PRIORITY["vehicle"]
    assert priorities["AppearanceCase"] == ITEM_ID_SOURCE_PRIORITY["appearance"]
    assert priorities["VehicleModuleCase"] == ITEM_ID_SOURCE_PRIORITY["vehicle_module"]
