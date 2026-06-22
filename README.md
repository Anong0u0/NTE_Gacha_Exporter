# NTE Gacha Exporter | 異環抽卡紀錄導出

繁體中文 | [English](https://github.com/Anong0u0/nte_gacha_exporter/blob/master/docs/README.en.md)

使用 Windows pktmon 擷取異環封包，匯出限定棋盤、標準棋盤、弧盤研募紀錄，產生 JSON/CSV。

## 特色

- 桌面介面與 Rust CLI 操作
- 自動翻頁協助擷取
- 匯出 JSON/CSV 格式資料
- 內建多語系輸出名稱：`de`、`en`、`es`、`fr`、`ja`、`ko`、`ru`、`zh-CN`、`zh-Hans`、`zh-Hant`
- 可選 assets pack 讓桌面 GUI 顯示抽卡項目與卡池圖片；未安裝時功能仍可正常使用

## 快速開始

1. 從 [GitHub Releases](https://github.com/Anong0u0/nte_gacha_exporter/releases) 下載最新 Windows portable package release zip，不要下載 Source code zip
2. 解壓縮整個資料夾
3. 開啟 `nte-gacha-exporter.exe`
4. 需要 GUI 圖片時，在 `Settings` -> `Assets Pack` 執行 `Check assets` 與 `Download assets`

桌面介面會將資料存入 portable `data/`，可再匯出 JSON/CSV。CLI 未指定輸出路徑時，匯出檔案會寫入目錄下的 `output/`。

Release 會同時附上 `nte-assets-pack-<version>-<maphash>.zip` 與 `nte-assets-pack-manifest.json`。一般使用不需要手動處理這兩個檔案；桌面程式會依目前更新通道下載相容版本。

## 系統需求

- Windows 10/11
- live capture 需要管理員權限，工具使用內建 pktmon runtime
- 已啟動的 NTE 遊戲
- 自動翻頁需要管理員權限、遊戲視窗處於前台可見、手動 F3 開啟抽卡頁面；建議使用 1920x1080；其他解析度可能錯誤

## 使用方式

開啟 `nte-gacha-exporter.exe` 後，設定輸出、語言與擷取選項；一般使用情境只需要保留預設值並開始擷取。

CLI capture 會在需要時要求管理員權限；使用自動翻頁前請讓遊戲停在抽卡主頁，且左下文件圖示與弧盤研募入口可見；流程完成後會停止擷取並寫出輸出；若自動翻頁失敗，工具會停止自動點擊並保留擷取狀態。

CLI:

```powershell
.\nte-gacha-exporter-cli.exe replay .\output\raw-260611-153012.jsonl --json .\output\history.json --csv .\output\history.csv
.\nte-gacha-exporter-cli.exe capture --output-raw --json .\output\history.json --csv .\output\history.csv
.\nte-gacha-exporter-cli.exe capture --auto-page --output-raw
.\nte-gacha-exporter-cli.exe doctor
.\nte-gacha-exporter-cli.exe maps build --assets-root D:\NTE_Assets --locale zh-Hant
```

## 輸出

Public JSON 只包含匯出資訊與 `nte.list` 紀錄：

```json
{
  "info": {
    "schema": "nte-gacha-exporter-export",
    "schema_version": "4.0",
    "privacy": "sanitized"
  },
  "nte": {
    "list": [
      {
        "record_id": "b4b36f5d...",
        "source_order": 0,
        "record_type": "monopoly",
        "time": "2026-04-30 17:02:15",
        "pool_id": "CardPool_Character",
        "pool_name": "王牌一代目",
        "item_id": "Fashion_vehicle_1010_V008",
        "item_name": "改裝件·萌虎來襲-塗裝",
        "count": 1,
        "roll_label": "3",
        "roll_points": 3
      }
    ]
  }
}
```

CSV 會依選用語系輸出本地化欄位名稱；除非已檢查內容，否則不要公開 raw capture 檔案。

## Assets Pack

主程式不內建圖片資源。assets pack 只包含目前內建 maps 引用到的圖片，來源固定為 [Waifus-Grace/NTE_Assets](https://github.com/Waifus-Grace/NTE_Assets) 的釘選 commit，release manifest 會記錄 app version、maps hash、來源 commit、zip sha256 與檔案數。

安裝位置在 portable 目錄的 `data/assets-pack/current`。GUI 只透過內部 `nteasset` protocol 讀取 `assets/*.webp`，不開放任意檔案讀取。移除 assets pack 只會停用目前安裝版本；抽卡紀錄、匯入匯出與更新功能不受影響。

## 疑難排解

### `pktmon capture requires Windows`

確認在 Windows 環境執行；Linux 只支援開發與 raw replay 測試，不支援 live capture。

### `pktmon capture requires administrator privilege`

用管理員權限重新開啟工具。

### 找不到 `HTGame.exe`

先啟動 NTE，確認遊戲仍在執行，再重新開啟 `nte-gacha-exporter.exe`。

### 沒有寫出紀錄

開啟遊戲內抽卡歷史紀錄畫面，讓遊戲送出相關封包；若仍沒有紀錄，切換網路環境或重新啟動遊戲後再試。

## 開發

```powershell
cargo xtask ci
cargo xtask quality
```

CI gate 可拆開執行：

```powershell
cargo fmt --all --check
cargo fmt --manifest-path tools/agent-smoke/Cargo.toml --check
cargo test --workspace
cargo test --manifest-path tools/agent-smoke/Cargo.toml
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --manifest-path tools/agent-smoke/Cargo.toml --all-targets -- -D warnings
pushd apps\desktop
bun install --frozen-lockfile
bun run typecheck
bun run build
popd
```

日常 quality gate 可拆開排查：

```powershell
cargo xtask check-long-code
cargo machete
pushd tools\agent-smoke
cargo machete
popd
pushd apps\desktop
bun install --frozen-lockfile
bun run knip --reporter compact
bun run typecheck -- --noUnusedLocals --noUnusedParameters --pretty false
popd
```

Rust workspace 位於 repo root，GUI app 位於 `apps/desktop/`，核心 crates 位於 `crates/`。Repo automation 位於 `tools/xtask`。
Map resources 位於 `crates/nte-assets/resources/maps`；用 Rust CLI 從 `NTE_Assets` 重建：

```powershell
cargo run -p nte-gacha-exporter-cli --bin nte-gacha-exporter-cli -- maps build --assets-root .local\NTE_Assets
```

Assets pack build:

```powershell
cargo run -p nte-gacha-exporter-cli --bin nte-gacha-exporter-cli -- assets pack build --assets-root D:\path\NTE_Assets --out dist\nte-assets-pack.zip
```

Windows release package:

```powershell
powershell.exe -ExecutionPolicy Bypass -File packaging\build-win-release.ps1
```

## Credits

- [Waifus-Grace/NTE_Assets](https://github.com/Waifus-Grace/NTE_Assets) for exported game assets and localization data.
- [rj0217/sniffbridge](https://github.com/rj0217/sniffbridge) by rj0217 for the MIT-licensed Windows PID/interface detection approach; only the minimal logic needed by this exporter is embedded.

## 授權

[MIT](https://github.com/Anong0u0/nte_gacha_exporter/blob/master/LICENSE)
