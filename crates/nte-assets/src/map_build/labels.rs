fn custom_csv_header(field: &str, locale: &str) -> Option<&'static str> {
    match (field, locale) {
        ("pool_name", "de" | "en") => Some("Pool"),
        ("pool_name", "es") => Some("Banner"),
        ("pool_name", "fr") => Some("Bannière"),
        ("pool_name", "ja") => Some("ガチャ"),
        ("pool_name", "ko") => Some("뽑기"),
        ("pool_name", "ru") => Some("Баннер"),
        ("pool_name", "zh-CN" | "zh-Hans" | "zh-Hant") => Some("卡池"),
        _ => None,
    }
}

fn csv_header_keys(field: &str) -> &'static [(&'static str, &'static str)] {
    match field {
        "time" => &[("ST_Ui", "BPUI_GashaponRecord_time")],
        "pool_group" => &[("ST_Ui", "BPUI_LotteryDiceRecord_qipanleixing")],
        "item_name" => &[
            ("ST_Ui", "BPUI_LotteryDiceRecord_daojumingcheng"),
            ("ST_Ui", "BPUI_GashaponRecord_Name"),
        ],
        "count" => &[
            ("ST_UI_hpy", "MangHe_09"),
            ("ST_UI_hpy", "MangHe_23"),
            ("ST_Ui", "BPUI_ConsumableUse_UseNumber"),
        ],
        "roll_label" => &[("ST_Ui", "BPUI_LotteryDiceRecord_touzhidianshu")],
        "secondary_item_name" => &[("ST_Ui", "BPUI_LotteryResult_AdditionalReward")],
        _ => &[],
    }
}

fn csv_header_joiner(locale: &str) -> &'static str {
    match locale {
        "ja" | "zh-CN" | "zh-Hans" | "zh-Hant" => "",
        _ => " ",
    }
}

fn csv_headers(localization: &Localization, locale: &str) -> JsonObject {
    let mut headers = BTreeMap::new();
    for field in [
        "time",
        "pool_group",
        "pool_name",
        "item_name",
        "count",
        "roll_label",
        "secondary_item_name",
        "secondary_count",
    ] {
        let mut text = custom_csv_header(field, locale).map(ToString::to_string);
        for (namespace, key) in csv_header_keys(field) {
            if text.is_some() {
                break;
            }
            text = clean_name(localized_key(localization, namespace, key));
        }
        headers.insert(field.to_string(), text.unwrap_or_else(|| field.to_string()));
    }
    if headers.get("secondary_item_name").map(String::as_str) != Some("secondary_item_name")
        && headers.get("count").map(String::as_str) != Some("count")
    {
        let joiner = csv_header_joiner(locale);
        let secondary_item_name = headers
            .get("secondary_item_name")
            .cloned()
            .unwrap_or_default();
        let count = headers.get("count").cloned().unwrap_or_default();
        headers.insert(
            "secondary_count".to_string(),
            format!("{secondary_item_name}{joiner}{count}"),
        );
    }
    let pool_header = custom_csv_header("pool_name", locale).map(ToString::to_string);
    let pool_type_header = clean_name(localized_key(
        localization,
        "ST_UI_C",
        "BPUI_CharacterEquipDevFilter_15",
    ));
    if let (Some(pool_header), Some(pool_type_header)) = (pool_header, pool_type_header) {
        let joiner = csv_header_joiner(locale);
        headers.insert(
            "pool_group".to_string(),
            format!("{pool_header}{joiner}{pool_type_header}"),
        );
    }
    string_map_value(headers)
}

fn build_labels(localization: &Localization) -> BTreeMap<String, String> {
    let label_keys = [
        ("Abyss_GamepadKeys_1", "ST_Ui", "Abyss_GamepadKeys_1"),
        ("AbyssClone_Award_02", "ST_Ui", "AbyssClone_Award_02"),
        (
            "BPUI_LotteryResult_jidianzengli",
            "ST_Ui",
            "BPUI_LotteryResult_jidianzengli",
        ),
        (
            "BPUI_LotteryResult_chenmiandi",
            "ST_Ui",
            "BPUI_LotteryResult_chenmiandi",
        ),
        (
            "BPUI_LotteryDiceRecord_biaozhunqipan",
            "ST_Ui",
            "BPUI_LotteryDiceRecord_biaozhunqipan",
        ),
        (
            "BPUI_LotteryDiceRecord_qipanleixing",
            "ST_Ui",
            "BPUI_LotteryDiceRecord_qipanleixing",
        ),
        (
            "BPUI_LotteryDiceRecord_xiandingqipan",
            "ST_Ui",
            "BPUI_LotteryDiceRecord_xiandingqipan",
        ),
        (
            "BPUI_LotteryModuleEntrance_Title",
            "ST_Ui",
            "BPUI_LotteryModuleEntrance_Title",
        ),
        ("TreasureBox_2", "ST_Ui", "TreasureBox_2"),
        (
            "UI_CloneSystemChallengeFailed_Retry",
            "ST_Ui",
            "UI_CloneSystemChallengeFailed_Retry",
        ),
        (
            "UI_CloneSystemStaminaTips_Enter",
            "ST_Ui",
            "UI_CloneSystemStaminaTips_Enter",
        ),
        (
            "UI_Lottery_GachaDetails_Zhitoujilu",
            "ST_Ui",
            "UI_Lottery_GachaDetails_Zhitoujilu",
        ),
        (
            "UI_Lottery_GachaDetails_title",
            "ST_Ui",
            "UI_Lottery_GachaDetails_title",
        ),
        (
            "UW_LotteryBase_BP_Hupanyanmu",
            "ST_Ui",
            "UW_LotteryBase_BP_Hupanyanmu",
        ),
        ("Mall_8_name", "ST_Ui", "Mall_8_name"),
        (
            "W_Vehicle_Button_Choose",
            "ST_Ui",
            "W_Vehicle_Button_Choose",
        ),
        ("W_HTButton_Next_Page", "ST_Ui", "W_HTButton_Next_Page"),
        ("common_3", "ST_Ui", "common_3"),
        ("ui_forkshop_03", "ST_Ui", "ui_forkshop_03"),
        ("ui_forkshop_07", "ST_Ui", "ui_forkshop_07"),
        ("ui_forkshop_10", "ST_Ui", "ui_forkshop_10"),
        ("ui_appearance_02", "ST_Ui", "ui_appearance_02"),
    ];
    label_keys
        .into_iter()
        .filter_map(|(label_id, namespace, key)| {
            clean_name(localized_key(localization, namespace, key))
                .map(|text| (label_id.to_string(), text))
        })
        .collect()
}

