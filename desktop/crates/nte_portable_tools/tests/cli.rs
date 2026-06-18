use std::path::PathBuf;
use std::process::Command;

use serde_json::{json, Value};

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_nte-gacha-cli"))
}

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tests/fixtures/sample.raw.jsonl")
}

#[test]
fn version_prints_package_version() {
    let output = Command::new(bin()).arg("--version").output().unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        env!("CARGO_PKG_VERSION")
    );
}

#[test]
fn replay_writes_public_outputs() {
    let tmp = tempfile::tempdir().unwrap();
    let json = tmp.path().join("history.json");
    let csv = tmp.path().join("history.csv");

    let output = Command::new(bin())
        .args([
            "replay",
            fixture().to_str().unwrap(),
            "--json",
            json.to_str().unwrap(),
            "--csv",
            csv.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let public = std::fs::read_to_string(json).unwrap();
    assert!(public.contains("\"schema\": \"nte-gacha-export\""));
    assert!(public.contains("\"nte\""));
    assert!(!public.contains("\"_debug\""));
    assert!(std::fs::read_to_string(csv).unwrap().contains("獲得時間"));
}

#[test]
fn replay_reports_bad_raw() {
    let tmp = tempfile::tempdir().unwrap();
    let raw = tmp.path().join("bad.raw.jsonl");
    std::fs::write(&raw, "{}\n").unwrap();

    let output = Command::new(bin())
        .args(["replay", raw.to_str().unwrap()])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("capture_start"));
}

#[test]
fn help_does_not_expose_removed_tui_or_debug_json() {
    let output = Command::new(bin()).arg("--help").output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("tui"));
    assert!(!stdout.contains("debug-json"));
}

#[test]
fn maps_build_writes_locale_map() {
    let tmp = tempfile::tempdir().unwrap();
    let assets = tmp.path().join("NTE_Assets");
    let out_dir = tmp.path().join("maps");
    write_minimal_assets(&assets);

    let output = Command::new(bin())
        .args([
            "maps",
            "build",
            "--assets-root",
            assets.to_str().unwrap(),
            "--locale",
            "zh-Hant",
            "--out-dir",
            out_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("zh-Hant: items=1 pools=2 labels=3"));
    let map: Value =
        serde_json::from_str(&std::fs::read_to_string(out_dir.join("zh-Hant.json")).unwrap())
            .unwrap();
    assert_eq!(map["schema_version"], 4);
    assert_eq!(map["items"]["1010"]["name"], "Character·Nanali");
    assert_eq!(
        map["banners"]["monopoly_standard"]["banner_type"],
        "standard"
    );
}

fn write_minimal_assets(root: &std::path::Path) {
    write_json(
        root.join("Localization/zh-Hant/game.json"),
        json!({
            "ST_Common": {
                "item_type_3": "Character"
            },
            "ST_Item": {
                "char_1010": "Nanali"
            },
            "ST_Ui": {
                "BPUI_LotteryDiceRecord_biaozhunqipan": "標準棋盤",
                "BPUI_LotteryDiceRecord_xiandingqipan": "限定棋盤",
                "UW_LotteryBase_BP_Hupanyanmu": "弧盤研募",
                "Lottery_Kachimingcheng_changzhu": "世間奇遇",
                "Lottery_Kachimingcheng_nanali": "王牌一代目"
            }
        }),
    );
    write_json(
        root.join("DataTable/Character/DT_Character.json"),
        json!({"Rows": {
            "1010": {
                "ItemName": {"Key": "char_1010", "TableId": "ST_Item"},
                "ItemQuality": "EItemQuality::ITEM_QUALITY_ORANGE"
            }
        }}),
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
}

fn write_json(path: PathBuf, value: Value) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
}
