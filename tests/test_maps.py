from __future__ import annotations

import json
from pathlib import Path

import pytest

from nte_gacha_exporter.mapping.assets import ITEM_ID_SOURCE_PRIORITY, _known_item_id_priorities, build_map
from nte_gacha_exporter.mapping.runtime import available_locales, load_map, load_map_file


def test_all_maps_have_expected_schema():
    for locale in available_locales():
        source_path = Path("src/nte_gacha_exporter/resources/maps") / f"{locale}.json"
        source = json.loads(source_path.read_text(encoding="utf-8"))
        assert set(source) == {
            "schema_version",
            "csv_headers",
            "items",
            "item_aliases",
            "pools",
            "banners",
            "gacha_rules",
            "labels",
        }
        assert source["schema_version"] == 4
        assert not ({"pool_meta", "pool_rules", "item_meta"} & set(source))
        for item_id, item in source["items"].items():
            assert {"name", "rarity", "category"} <= set(item)
            assert "item_id" not in item
            assert item["name"]
            assert isinstance(item["rarity"], int)
            assert item["category"]
            if "asset_refs" in item:
                for ref in item["asset_refs"].values():
                    assert isinstance(ref, str)
                    assert ref.startswith("/Game/")
            if item_id == "1010":
                assert item["asset_refs"]["portrait"].startswith("/Game/")
                assert item["color"].startswith("#")
        for pool in source["pools"].values():
            assert "pool_id" not in pool
            assert "subtitle" not in pool
            assert pool["name"]
        for pool_id, pool in source["pools"].items():
            if pool_id.startswith("ForkLottery_"):
                assert pool["pickup_item_ids"]
                assert pool["banner_ids"] == [pool_id]
        assert source["banners"]["ForkLottery_AnHunQu"]["source"]["confidence"] == "exact"
        assert source["banners"]["ForkLottery_AnHunQu"]["rate_up_5"]
        assert source["banners"]["monopoly_limited_AnHunQu"]["source"]["confidence"] == "curated"
        assert source["banners"]["monopoly_limited_AnHunQu"]["rate_up_5"] == ["1004"]
        assert source["banners"]["monopoly_limited_Nanali"]["phase"] == "limited_2026_05_13"
        assert "version" not in source["banners"]["monopoly_limited_Nanali"]
        banner_ids = set(source["banners"])
        for candidate_path in Path("src/nte_gacha_exporter/resources/maps").glob("*.json"):
            candidate = json.loads(candidate_path.read_text(encoding="utf-8"))
            assert set(candidate["banners"]) == banner_ids
            for banner_id, banner in source["banners"].items():
                candidate_banner = candidate["banners"][banner_id]
                assert candidate_banner.get("version") == banner.get("version")
                assert candidate_banner.get("phase") == banner.get("phase")
        assert source["gacha_rules"]["fork_lottery_s"]["hard_pity_5"] == 80

        data = load_map(locale)
        item_ids = set(data["items"])
        meta_ids = {item["item_id"] for item in data["item_meta"]}
        assert item_ids == meta_ids
        assert not (set(data["item_aliases"]) & item_ids)
        assert set(data["item_aliases"].values()) <= item_ids
        assert data["banners"]["monopoly_standard"]["banner_type"] == "standard"
        assert data["banners"]["monopoly_limited_Nanali"]["phase"] == "limited_2026_05_13"
        assert data["gacha_rules"]["monopoly_limited"]["hard_pity_5"] == 90


def test_map_validation_rejects_banner_id_pool_mismatch(tmp_path):
    data = _minimal_banner_map()
    data["pools"]["CardPool_NewRole"]["banner_ids"] = ["monopoly_limited_Nanali"]
    path = tmp_path / "bad.json"
    path.write_text(json.dumps(data, ensure_ascii=False), encoding="utf-8")

    with pytest.raises(ValueError, match="other pools"):
        load_map_file(path)


def test_map_validation_rejects_overlapping_limited_windows(tmp_path):
    data = _minimal_banner_map()
    data["banners"]["monopoly_limited_Nanali"]["start_at"] = "2026-05-10 00:00:00"
    data["banners"]["monopoly_limited_Xun"]["start_at"] = "2026-05-12 00:00:00"
    path = tmp_path / "bad.json"
    path.write_text(json.dumps(data, ensure_ascii=False), encoding="utf-8")

    with pytest.raises(ValueError, match="overlap"):
        load_map_file(path)


def test_map_validation_rejects_invalid_gacha_rule_values(tmp_path):
    cases = [
        ("hard_pity_5", 0, "positive"),
        ("pickup_win_rate_5", 101, "0..100"),
        ("pool_kind", "unknown_pool_kind", "one of"),
    ]
    for field, value, message in cases:
        data = _minimal_banner_map()
        data["gacha_rules"]["monopoly_limited"][field] = value
        path = tmp_path / f"bad-{field}.json"
        path.write_text(json.dumps(data, ensure_ascii=False), encoding="utf-8")

        with pytest.raises(ValueError, match=message):
            load_map_file(path)


def test_map_validation_accepts_banner_version_phase_machine_ids(tmp_path):
    data = _minimal_banner_map()
    data["banners"]["monopoly_limited_Nanali"]["version"] = "v1.0"
    data["banners"]["monopoly_limited_Nanali"]["phase"] = "limited_2026_05_13"
    path = tmp_path / "good.json"
    path.write_text(json.dumps(data, ensure_ascii=False), encoding="utf-8")

    loaded = load_map_file(path)

    assert loaded["banners"]["monopoly_limited_Nanali"]["version"] == "v1.0"
    assert loaded["banners"]["monopoly_limited_Nanali"]["phase"] == "limited_2026_05_13"


def test_map_validation_rejects_rate_up_item_without_domain_type(tmp_path):
    data = _minimal_banner_map()
    del data["items"]["1010"]["domain_type"]
    path = tmp_path / "bad-rate-up-domain.json"
    path.write_text(json.dumps(data, ensure_ascii=False), encoding="utf-8")

    with pytest.raises(ValueError, match="rate_up item ids must have domain_type"):
        load_map_file(path)


def test_map_validation_rejects_invalid_banner_version_phase_values(tmp_path):
    cases = [
        ("phase", "", "machine id"),
        ("phase", "phase 1", "machine id"),
        ("version", 123, "string or null"),
        ("phase", "王牌一代目", "machine id"),
    ]
    for field, value, message in cases:
        data = _minimal_banner_map()
        data["banners"]["monopoly_limited_Nanali"][field] = value
        path = tmp_path / f"bad-{field}.json"
        path.write_text(json.dumps(data, ensure_ascii=False), encoding="utf-8")

        with pytest.raises(ValueError, match=message):
            load_map_file(path)


def _minimal_banner_map() -> dict[str, object]:
    return {
        "schema_version": 4,
        "csv_headers": {},
        "items": {
            "1010": {"name": "Nanali", "rarity": 5, "category": "character", "domain_type": "character"},
            "1052": {"name": "Xun", "rarity": 5, "category": "character", "domain_type": "character"},
        },
        "item_aliases": {},
        "pools": {
            "CardPool_Character": {
                "name": "限定棋盤",
                "banner_ids": ["monopoly_limited_Nanali", "monopoly_limited_Xun"],
            },
            "CardPool_NewRole": {"name": "標準棋盤", "banner_ids": ["monopoly_standard"]},
        },
        "banners": {
            "monopoly_limited_Nanali": {
                "banner_id": "monopoly_limited_Nanali",
                "pool_id": "CardPool_Character",
                "pool_kind": "monopoly_limited",
                "banner_type": "limited",
                "title": "王牌一代目",
                "end_at": "2026-05-13 05:59:00",
                "timezone": "Asia/Shanghai",
                "rate_up_5": ["1010"],
                "rate_up_4": [],
                "rule_id": "monopoly_limited",
                "source": {"confidence": "curated", "tables": []},
            },
            "monopoly_limited_Xun": {
                "banner_id": "monopoly_limited_Xun",
                "pool_id": "CardPool_Character",
                "pool_kind": "monopoly_limited",
                "banner_type": "limited",
                "title": "獨酌朧月流",
                "start_at": "2026-05-13 05:59:00",
                "end_at": "2026-06-03 05:59:00",
                "timezone": "Asia/Shanghai",
                "rate_up_5": ["1052"],
                "rate_up_4": [],
                "rule_id": "monopoly_limited",
                "source": {"confidence": "curated", "tables": []},
            },
            "monopoly_standard": {
                "banner_id": "monopoly_standard",
                "pool_id": "CardPool_NewRole",
                "pool_kind": "monopoly_standard",
                "banner_type": "standard",
                "title": "世間奇遇",
                "rate_up_5": [],
                "rate_up_4": [],
                "rule_id": "monopoly_standard",
                "source": {"confidence": "curated", "tables": []},
            },
        },
        "gacha_rules": {
            "monopoly_limited": {
                "rule_id": "monopoly_limited",
                "pool_kind": "monopoly_limited",
                "source": {"confidence": "curated", "tables": []},
            },
            "monopoly_standard": {
                "rule_id": "monopoly_standard",
                "pool_kind": "monopoly_standard",
                "source": {"confidence": "curated", "tables": []},
            },
        },
        "labels": {},
    }


def test_zh_hant_key_mappings():
    data = load_map("zh-Hant")
    assert data["items"]["Fashion_vehicle_1010_V008"] == "改裝件·萌虎來襲-塗裝"
    assert data["items"]["Fashion_vehicle_1052_V024"] == "改裝件·秋色殘影-塗裝"
    assert data["items"]["Fashion_character_1004_01"] == "時裝·鎏金交響詩"
    assert data["items"]["Characterawaken_daffodill"] == "道具·心象碎片·達芙蒂爾"
    assert data["items"]["DiceNormal"] == "道具·捏造骰子"
    assert "Characterawaken_dafudier" not in data["items"]
    assert "DIceNormal" not in data["items"]
    assert "DIceLimite" not in data["items"]
    assert "Fashion_glide_1004" not in data["items"]
    assert "Fork_TigerTally" not in data["items"]
    assert "fork_Vine" not in data["items"]
    assert data["item_aliases"]["Characterawaken_dafudier"] == "Characterawaken_daffodill"
    assert data["item_aliases"]["DIceNormal"] == "DiceNormal"
    assert data["item_aliases"]["DIceLimite"] == "Dicelimite"
    assert data["item_aliases"]["Fashion_glide_1004"] == "Fashion_Glide_1004"
    assert data["item_aliases"]["Fork_TigerTally"] == "fork_TigerTally"
    assert data["item_aliases"]["fork_Vine"] == "fork_vine"
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
        "banner_ids": ["monopoly_standard"],
    }
    assert data["pool_meta"]["CardPool_Character"] == {
        "group_label": "限定棋盤",
        "title_windows": [
            {"end_at_tz8": "2026-05-13 05:59:00", "title": "王牌一代目"},
            {"end_at_tz8": "2026-06-03 05:59:00", "title": "獨酌朧月流"},
            {"end_at_tz8": "2026-06-24 05:59:00", "title": "久夢初醒時"},
            {"end_at_tz8": "2026-07-08 05:59:00", "title": "無歸路"},
        ],
        "banner_ids": [
            "monopoly_limited_AnHunQu",
            "monopoly_limited_Kaesi",
            "monopoly_limited_Nanali",
            "monopoly_limited_Xun",
        ],
    }
    assert data["pool_meta"]["ForkLottery_AnHunQu"] == {
        "group_label": "弧盤研募",
        "title": "夜曲特刊",
        "pickup_item_ids": ["fork_Rose"],
        "asset_refs": {
            "background": "/Game/UI/UI/ForkShop/UI_YH_Shoppingmall_hupandibanahqbg.UI_YH_Shoppingmall_hupandibanahqbg",
            "icon": "/Game/UI/UI_Icon/Fork/1024/fork_Rose.fork_Rose",
        },
        "banner_ids": ["ForkLottery_AnHunQu"],
    }
    fork_rule = next(rule for rule in data["pool_rules"] if rule["pool_id"] == "ForkLottery_AnHunQu")
    vehicle_item = next(item for item in data["item_meta"] if item["item_id"] == "Fashion_vehicle_1010_V008")
    assert fork_rule == {
        "pool_id": "ForkLottery_AnHunQu",
        "pool_name": "奇蹟盒盒",
        "group_label": "弧盤研募",
        "pickup_item_ids": ["fork_Rose"],
    }
    assert set(vehicle_item) == {
        "item_id",
        "item_name",
        "rarity",
        "category",
        "domain_type",
        "subtype",
        "asset_refs",
        "color",
    }
    assert vehicle_item["rarity"] == 5
    assert any(rule["pool_id"] == "CardPool_Character" for rule in data["pool_rules"])
    assert data["banners"]["ForkLottery_AnHunQu"]["currency_id"] == "WeaponGacha"
    assert data["banners"]["monopoly_limited_Nanali"]["rate_up_5"] == ["1010"]
    assert data["gacha_rules"]["fork_lottery_s"]["rule_text_refs"]["probability_desc"] == "ui_forkshop_12_anhunqu"
    rose_item = next(item for item in data["item_meta"] if item["item_id"] == "fork_Rose")
    dice_item = next(item for item in data["item_meta"] if item["item_id"] == "DiceNormal")
    assert rose_item["asset_refs"] == {
        "head_icon": "/Game/UI/UI_Icon/Fork/fork_Rose_100.fork_Rose_100",
        "icon": "/Game/UI/UI_Icon/Fork/fork_Rose_256.fork_Rose_256",
    }
    assert dice_item["asset_refs"] == {
        "head_icon": "/Game/UI/UI_Icon/item_100/DiceNormal_100.DiceNormal_100",
        "icon": "/Game/UI/UI_Icon/item_100/DiceNormal_256.DiceNormal_256",
        "portrait": "/Game/UI/UI_Icon/Item/DiceNormal.DIceNormal",
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
                "Characterawaken_daffodill_name": "心象碎片·達芙蒂爾",
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
                    "fork_jianang": {
                        "HeadIcon": {"AssetPathName": "/Game/UI/UI_Icon/Fork/1024/fork_jianang.fork_jianang"},
                        "ItemIcon": {"AssetPathName": "/Game/UI/UI_Icon/Fork/fork_jianang_256.fork_jianang_256"},
                        "OutlineColor": {"Hex": "aabbcc"},
                    },
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
                    "Characterawaken_daffodill": {
                        "ItemType": "EItemType::ITEM_TYPE_Lottery",
                        "ItemQuality": "EItemQuality::ITEM_QUALITY_ORANGE",
                        "ItemName": {
                            "TableId": "/Game/Text/ST_Item.ST_Item",
                            "Key": "Characterawaken_daffodill_name",
                        },
                    },
                    "DiceNormal": {
                        "ItemType": "EItemType::ITEM_TYPE_Lottery",
                        "ItemQuality": "EItemQuality::ITEM_QUALITY_ORANGE",
                        "ItemName": {"TableId": "/Game/Text/ST_Item.ST_Item", "Key": "DiceNormal_Name"},
                    },
                    "Fashion_character_1004_01": {
                        "ItemType": "EItemType::ITEM_TYPE_FASHION",
                        "ItemQuality": "EItemQuality::ITEM_QUALITY_ORANGE",
                        "ItemName": {"TableId": "/Game/Text/ST_Appearance.ST_Appearance", "Key": "Fashion_1004_1_Name"},
                    },
                    "Fashion_glide_1004": {
                        "ItemType": "EItemType::ITEM_TYPE_GLIDE",
                        "ItemQuality": "EItemQuality::ITEM_QUALITY_BLUE",
                        "ItemName": {"TableId": "/Game/Text/ST_Item.ST_Item", "Key": "Fashion_glide_1004_name"},
                    },
                    "EventTokenA": {
                        "ItemType": "EItemType::ITEM_TYPE_EVENT_TOKEN",
                        "ItemQuality": "EItemQuality::ITEM_QUALITY_BLUE",
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
                        "SSRItems": [
                            {"ItemID": "fork_jianang", "bIsShowIconTip": False},
                            {"ItemID": "Fashion_vehicle_1010_V008"},
                        ],
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
                        "Quality": "EItemQuality::ITEM_QUALITY_ORANGE",
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
                    "fork_dustbin": {
                        "Quality": "EItemQuality::ITEM_QUALITY_ORANGE",
                        "ItemName": {"TableId": "/Game/Text/ST_Item.ST_Item", "Key": "fork_dustbin_name"},
                    },
                    "fork_jianang": {
                        "Quality": "EItemQuality::ITEM_QUALITY_ORANGE",
                        "ItemName": {"TableId": "/Game/Text/ST_Item.ST_Item", "Key": "fork_jianang_name"},
                    },
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
                        "UpList": ["fork_jianang"],
                        "Bg": {"AssetPathName": "/Game/UI/UI/ForkShop/test_bg.test_bg"},
                        "Icon": {"AssetPathName": "/Game/UI/UI_Icon/Fork/1024/fork_jianang.fork_jianang"},
                        "CurrencyID": "EventTokenA",
                        "CurrencyCnt": 2,
                        "OnceLotteryCnt": 10,
                        "UpGuaranteeCnt": 80,
                        "RuleDesc1": {"TableId": "/Game/Text/ST_Ui.ST_Ui", "Key": "rule_1"},
                        "RuleDesc2": {"TableId": "/Game/Text/ST_Ui.ST_Ui", "Key": "rule_2"},
                        "ProbDesc": {"TableId": "/Game/Text/ST_Ui.ST_Ui", "Key": "prob_1"},
                    },
                }
            }
        ],
    )

    data = build_map(tmp_path, "zh-Hant")

    assert set(data) == {
        "schema_version",
        "csv_headers",
        "items",
        "item_aliases",
        "pools",
        "banners",
        "gacha_rules",
        "labels",
    }
    assert data["schema_version"] == 4
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
    assert data["items"]["Characterawaken_daffodill"] == {
        "name": "道具·心象碎片·達芙蒂爾",
        "rarity": 5,
        "category": "item",
        "domain_type": "item",
    }
    assert data["items"]["DiceNormal"]["name"] == "道具·捏造骰子"
    assert data["items"]["EventTokenA"]["name"] == "活動票券·測試票券"
    assert data["items"]["Fashion_character_1004_01"]["name"] == "時裝·鎏金交響詩"
    assert data["items"]["Fashion_Glide_1004"]["name"] == "滑翔翼·好柿成雙"
    assert data["items"]["Fashion_vehicle_1010_V008"]["name"] == "改裝件·萌虎來襲-塗裝"
    assert data["items"]["fork_dustbin"]["name"] == "弧盤·危險遊戲"
    assert data["items"]["fork_jianang"] == {
        "name": "弧盤·佳釀",
        "rarity": 5,
        "category": "fork",
        "domain_type": "fork",
        "asset_refs": {
            "head_icon": "/Game/UI/UI_Icon/Fork/1024/fork_jianang.fork_jianang",
            "icon": "/Game/UI/UI_Icon/Fork/1024/fork_jianang.fork_jianang",
            "portrait": "/Game/UI/UI_Icon/Fork/fork_jianang_256.fork_jianang_256",
        },
        "color": "#AABBCC",
    }
    assert "Characterawaken_dafudier" not in data["items"]
    assert "DIceNormal" not in data["items"]
    assert "Fashion_glide_1004" not in data["items"]
    assert data["item_aliases"] == {
        "Characterawaken_dafudier": "Characterawaken_daffodill",
        "DIceNormal": "DiceNormal",
        "Fashion_glide_1004": "Fashion_Glide_1004",
    }
    assert "Annulith" not in data["items"]
    assert "fork_missing" not in data["items"]
    assert data["pools"]["CardPool_NewRole"] == {
        "name": "標準棋盤",
        "group_label": "標準棋盤",
        "title": "世間奇遇",
        "banner_ids": ["monopoly_standard"],
    }
    assert data["pools"]["CardPool_Character"] == {
        "name": "限定棋盤",
        "group_label": "限定棋盤",
        "title_windows": [
            {"end_at_tz8": "2026-05-13 05:59:00", "title": "王牌一代目"},
            {"end_at_tz8": "2026-06-03 05:59:00", "title": "獨酌朧月流"},
            {"end_at_tz8": "2026-06-24 05:59:00", "title": "久夢初醒時"},
            {"end_at_tz8": "2026-07-08 05:59:00", "title": "無歸路"},
        ],
        "banner_ids": [
            "monopoly_limited_AnHunQu",
            "monopoly_limited_Kaesi",
            "monopoly_limited_Nanali",
            "monopoly_limited_Xun",
        ],
    }
    assert data["pools"]["1"] == {"name": "奇蹟盒盒"}
    assert data["pools"]["ForkLottery_AnHunQu"] == {
        "name": "奇蹟盒盒",
        "group_label": "弧盤研募",
        "title": "夜曲特刊",
        "pickup_item_ids": ["fork_jianang"],
        "asset_refs": {
            "background": "/Game/UI/UI/ForkShop/test_bg.test_bg",
            "icon": "/Game/UI/UI_Icon/Fork/1024/fork_jianang.fork_jianang",
        },
        "banner_ids": ["ForkLottery_AnHunQu"],
    }
    assert data["banners"]["ForkLottery_AnHunQu"] == {
        "banner_id": "ForkLottery_AnHunQu",
        "pool_id": "ForkLottery_AnHunQu",
        "pool_kind": "fork_lottery",
        "banner_type": "fork",
        "title": "夜曲特刊",
        "rate_up_5": ["fork_jianang"],
        "rate_up_4": [],
        "rule_id": "fork_lottery_s",
        "source": {"confidence": "exact", "tables": ["DataTable/Fork/DT_ForkLotteryPoolData.json"]},
        "asset_refs": {
            "background": "/Game/UI/UI/ForkShop/test_bg.test_bg",
            "icon": "/Game/UI/UI_Icon/Fork/1024/fork_jianang.fork_jianang",
        },
        "currency_id": "EventTokenA",
        "currency_count": 2,
        "roll_unit": 10,
    }
    assert data["banners"]["monopoly_limited_Nanali"]["phase"] == "limited_2026_05_13"
    assert "version" not in data["banners"]["monopoly_limited_Nanali"]
    assert (
        "Version/phase metadata is curated when present."
        in data["banners"]["monopoly_limited_Nanali"]["source"]["notes"]
    )
    assert data["gacha_rules"]["fork_lottery_s"]["hard_pity_5"] == 80
    assert data["gacha_rules"]["fork_lottery_s"]["rule_text_refs"] == {
        "rule_desc_1": "rule_1",
        "rule_desc_2": "rule_2",
        "probability_desc": "prob_1",
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
                        "ItemQuality": "ITEM_QUALITY_ORANGE",
                        "ItemType": "EItemType::ITEM_TYPE_Lottery",
                        "ItemName": {"TableId": "/Game/Text/ST_Item.ST_Item", "Key": "DiceNormal_Name"},
                    },
                }
            }
        ],
    )

    data = build_map(tmp_path, "missing-locale")

    assert data["items"]["DiceNormal"]["name"] == "Item·Test Dice"


def test_build_map_falls_back_to_up_show_rewards_for_fork_pickup(tmp_path):
    def write_json(relative: str, data: object) -> None:
        path = tmp_path / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(data, ensure_ascii=False), encoding="utf-8")

    write_json(
        "Localization/zh-Hant/game.json",
        {
            "ST_Common": {"item_type_5": "弧盤"},
            "ST_Item": {"fork_jianang_name": "佳釀"},
            "ST_Ui": {
                "UW_LotteryBase_BP_Hupanyanmu": "弧盤研募",
                "ui_forkshop_24": "奇蹟盒盒",
                "ui_forklottery_up_anhunqu_01": "夜曲特刊",
            },
        },
    )
    write_json(
        "DataTable/Fork/DT_ForkItemData.json",
        [
            {
                "Rows": {
                    "fork_jianang": {
                        "Quality": "EItemQuality::ITEM_QUALITY_ORANGE",
                        "ItemName": {"TableId": "/Game/Text/ST_Item.ST_Item", "Key": "fork_jianang_name"},
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
                    "ForkLottery_AnHunQu": {
                        "Name": {"TableId": "/Game/Text/ST_Ui.ST_Ui", "Key": "ui_forkshop_24"},
                        "ShowText1": {"TableId": "/Game/Text/ST_Ui.ST_Ui", "Key": "ui_forklottery_up_anhunqu_01"},
                        "ShowRewards": [{"ItemID": "fork_jianang", "IsUp": True}],
                    }
                }
            }
        ],
    )

    data = build_map(tmp_path, "zh-Hant")

    assert data["pools"]["ForkLottery_AnHunQu"]["pickup_item_ids"] == ["fork_jianang"]


def test_build_map_errors_when_fork_pickup_source_missing(tmp_path):
    def write_json(relative: str, data: object) -> None:
        path = tmp_path / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(data, ensure_ascii=False), encoding="utf-8")

    write_json("Localization/zh-Hant/game.json", {"ST_Ui": {"ui_forkshop_24": "奇蹟盒盒"}})
    write_json(
        "DataTable/Fork/DT_ForkLotteryPoolData.json",
        [
            {
                "Rows": {
                    "ForkLottery_AnHunQu": {
                        "Name": {"TableId": "/Game/Text/ST_Ui.ST_Ui", "Key": "ui_forkshop_24"},
                        "ShowRewards": [{"ItemID": "fork_jianang", "IsUp": False}],
                    }
                }
            }
        ],
    )

    with pytest.raises(ValueError, match="ForkLottery_AnHunQu"):
        build_map(tmp_path, "zh-Hant")


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
