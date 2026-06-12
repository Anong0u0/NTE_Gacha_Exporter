from __future__ import annotations

DEFAULT_UI_LANGUAGE = "en"

TEXT = {
    "zh-Hant": {
        "title": "NTE Gacha Exporter",
        "subtitle": "數字選單 / L 語言 / Q 離開",
        "capture": "擷取",
        "live": "即時擷取",
        "auto": "自動翻頁擷取",
        "autoPage": "自動翻頁",
        "replay": "重播 raw JSONL",
        "rawExport": "匯出 raw JSONL",
        "advanced": "進階工具",
        "settings": "設定",
        "start": "開始",
        "quit": "離開",
        "back": "返回",
        "reset": "重置設定",
        "choice": "選擇",
        "actions": "操作",
        "files": "檔案",
        "rawFile": "raw JSONL",
        "selected": "已選擇",
        "notSelected": "未選擇",
        "language": "介面語言",
        "languageSwitch": "Language switch to English",
        "recordLocale": "紀錄語系",
        "outputDir": "輸出目錄",
        "saveRaw": "保存 raw JSONL",
        "writeDebug": "輸出 debug JSON",
        "enabled": "開",
        "disabled": "關",
        "pressEnter": "按 Enter 返回",
        "pressAnyKey": "按任意鍵返回",
        "running": "執行中",
        "runningHint": "按 q 或 Ctrl+C 停止",
        "autoPrereq": (
            "開始前確認：遊戲視窗可見、停在抽卡主頁、建議 1920x1080；"
            "非 1920x1080 自動翻頁可能錯誤。自動翻頁會在啟動時要求管理員權限。"
        ),
        "autoConfirmPrompt": "選擇 [Enter=開始, B=返回]> ",
        "startedAdmin": "已要求管理員權限並重新啟動。",
        "waitingAdmin": "等待管理員視窗回傳...",
        "notStarted": "未開始。",
        "error": "錯誤",
        "done": "完成",
        "paths": "輸出",
        "lastRecords": "最後紀錄",
        "noRaw": "找不到 output/*.jsonl，請手動輸入路徑。",
        "newestOnly": "只顯示最新 9 筆。",
        "manualPath": "手動輸入路徑",
        "assetsRoot": "assets-root",
        "mapsLocale": "locale",
        "mapsOutDir": "out-dir",
        "defaultValue": "預設",
        "doctor": "環境檢查",
        "interfaces": "介面列表",
        "mapsList": "列出 maps",
        "mapsBuild": "建立 maps",
        "mainPrompt": "選擇 [1,2,L,Q]> ",
        "capturePrompt": "選擇 [1,2,3,4,S,R,B]> ",
        "rawExportPrompt": "選擇 [1,2,3,4,S,R,B]> ",
        "advancedPrompt": "選擇 [1,2,3,4,5,B]> ",
        "rawFilePrompt": "選擇 [1-9,M,B]> ",
        "inputPrompt": "正在輸入{field} [{current}]，Enter 確認，Esc 取消> ",
        "cancelled": "已取消。",
        "requestAdmin": "正在要求 {purpose} 的管理員權限。",
    },
    "en": {
        "title": "NTE Gacha Exporter",
        "subtitle": "Number menu / L language / Q quit",
        "capture": "Capture",
        "live": "Live capture",
        "auto": "Auto-page capture",
        "autoPage": "Auto page",
        "replay": "Replay raw JSONL",
        "rawExport": "Export raw JSONL",
        "advanced": "Advanced tools",
        "settings": "Settings",
        "start": "Start",
        "quit": "Quit",
        "back": "Back",
        "reset": "Reset settings",
        "choice": "Choice",
        "actions": "Actions",
        "files": "Files",
        "rawFile": "raw JSONL",
        "selected": "selected",
        "notSelected": "not selected",
        "language": "UI language",
        "languageSwitch": "語言切換成中文",
        "recordLocale": "Record locale",
        "outputDir": "Output directory",
        "saveRaw": "Save raw JSONL",
        "writeDebug": "Write debug JSON",
        "enabled": "on",
        "disabled": "off",
        "pressEnter": "Press Enter to return",
        "pressAnyKey": "Press any key to return",
        "running": "running",
        "runningHint": "Press q or Ctrl+C to stop",
        "autoPrereq": (
            "Before start: keep game visible, stay on gacha home, use 1920x1080 when possible. "
            "Non-1920x1080 auto page may be inaccurate. Admin permission is requested when needed."
        ),
        "autoConfirmPrompt": "Choice [Enter=go, B=back]> ",
        "startedAdmin": "Administrator permission requested; relaunched.",
        "waitingAdmin": "Waiting for administrator window...",
        "notStarted": "Not started.",
        "error": "Error",
        "done": "Done",
        "paths": "Outputs",
        "lastRecords": "Last records",
        "noRaw": "No output/*.jsonl found; enter a path manually.",
        "newestOnly": "Showing the newest 9 files.",
        "manualPath": "Manual path",
        "assetsRoot": "assets-root",
        "mapsLocale": "locale",
        "mapsOutDir": "out-dir",
        "defaultValue": "default",
        "doctor": "Doctor",
        "interfaces": "Interfaces",
        "mapsList": "List maps",
        "mapsBuild": "Build maps",
        "mainPrompt": "Choice [1,2,L,Q]> ",
        "capturePrompt": "Choice [1,2,3,4,S,R,B]> ",
        "rawExportPrompt": "Choice [1,2,3,4,S,R,B]> ",
        "advancedPrompt": "Choice [1,2,3,4,5,B]> ",
        "rawFilePrompt": "Choice [1-9,M,B]> ",
        "inputPrompt": "Entering {field} [{current}], Enter confirm, Esc cancel> ",
        "cancelled": "Cancelled.",
        "requestAdmin": "Requesting administrator permission for {purpose}.",
    },
}


class TuiI18n:
    def __init__(self, locale: str | None = None) -> None:
        self.locale = locale if locale in TEXT else DEFAULT_UI_LANGUAGE

    def text(self, key: str) -> str:
        return TEXT[self.locale].get(key, TEXT[DEFAULT_UI_LANGUAGE].get(key, key))

    def format(self, key: str, **values: object) -> str:
        return self.text(key).format(**values)


def validateI18nKeys() -> None:
    expected = set(TEXT[DEFAULT_UI_LANGUAGE])
    for locale, values in TEXT.items():
        actual = set(values)
        missing = expected - actual
        extra = actual - expected
        if missing or extra:
            raise ValueError(f"i18n keys mismatch for {locale}: missing={sorted(missing)} extra={sorted(extra)}")
