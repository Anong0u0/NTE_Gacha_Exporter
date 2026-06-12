# NTE Gacha Exporter | 異環抽卡紀錄導出

[繁體中文](README.md) | [English](https://github.com/Anong0u0/nte_gacha_exporter/blob/master/docs/README.en.md)

使用Npcap抓取異環封包，匯出 限定棋盤、標準棋盤、弧盤研募，產生 JSON/CSV

## 特色

- 互動式操作，自動翻頁協助擷取
- 匯出 JSON/CSV 格式資料
- 內建多語系輸出名稱：`de`、`en`、`es`、`fr`、`ja`、`ko`、`ru`、`zh-CN`、`zh-Hans`、`zh-Hant`

## 快速開始

1. 從 [GitHub Releases](https://github.com/Anong0u0/nte_gacha_exporter/releases) 下載最新 release zip
2. 解壓縮整個資料夾
3. 開啟 `nte-gacha.exe`

按1開始擷取匯出流程；`L` 切換繁中/英文，`Q` 離開
未指定輸出路徑時，匯出檔案會寫入目錄下的 `output/`

## 系統需求

- 需安裝[Npcap](https://npcap.com/#download)
 - 確保 `Install Npcap in WinPcap API-compatible mode` 已勾選
 - WiFi 使用者注意: 安裝 Npcap 時, 確保 `Support raw 802.11 traffic (and monitor mode) for wireless adapters` 已勾選
- 已啟動的 NTE 遊戲
- 自動翻頁需要管理員權限、遊戲視窗處於前台可見、手動F3開啟抽卡頁面；建議使用 1920x1080；其他解析度可能錯誤

## 使用方式

開啟 `nte-gacha.exe` 後，設定輸出、語言與擷取選項；一般使用情境只需要保留預設值並開始擷取

自動翻頁會在需要時要求管理員權限；使用前請讓遊戲停在抽卡主頁，且左下文件圖示與弧盤研募入口可見；流程完成後會停止擷取並寫出輸出；若自動翻頁失敗，工具會停止自動點擊並保留擷取狀態

## 輸出

Public JSON 只包含匯出資訊與 `nte.list` 紀錄：

```json
{
  "info": {
    "schema": "nte-gacha-export",
    "schema_version": "1.0",
    "privacy": "sanitized"
  },
  "nte": {
    "list": [
      {
        "record_type": "monopoly",
        "time": "2026-04-30 17:02:15",
        "pool_name": "王牌一代目",
        "item_name": "改裝件·萌虎來襲-塗裝",
        "count": 1
      }
    ]
  }
}
```

CSV 會依選用語系輸出本地化欄位名稱；除非已檢查內容，否則不要公開 raw capture 檔案

## 疑難排解

### `live capture requires Windows + Npcap`

確保是 Windows 且已安裝 Npcap 後重新開啟工具

### 找不到 `HTGame.exe`

先啟動 NTE，確認遊戲仍在執行，再重新開啟 `nte-gacha.exe`

### 沒有寫出紀錄

開啟遊戲內抽卡歷史紀錄畫面，讓遊戲送出相關封包；若仍沒有紀錄，切換網路環境或重新啟動遊戲後再試

## 開發

```powershell
poetry install --extras live
poetry run pytest
poetry run ruff check .
poetry run python packaging\nuitka\build.py
```

Package metadata 位於 `pyproject.toml`；執行 `poetry run nte-gacha`

## Credits

- [Npcap](https://npcap.com/) for Windows packet capture.
- [Waifus-Grace/NTE_Assets](https://github.com/Waifus-Grace/NTE_Assets) for exported game assets and localization data.
- [rj0217/sniffbridge](https://github.com/rj0217/sniffbridge) by rj0217 for the MIT-licensed Windows PID/interface detection approach; only the minimal logic needed by this exporter is embedded.

## 授權

[MIT](https://github.com/Anong0u0/nte_gacha_exporter/blob/master/LICENSE)
