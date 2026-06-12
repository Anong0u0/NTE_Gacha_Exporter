from __future__ import annotations

import json
import tempfile
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

from nte_gacha_exporter.mapping.runtime import DEFAULT_LOCALE
from nte_gacha_exporter.runtime import is_frozen, runtime_root

SETTINGS_FILENAME = "tui-settings.json"
UI_LANGUAGES = {"zh-Hant", "en"}


class SettingsAdminRelaunch(RuntimeError):
    """Raised after requesting an elevated settings write."""


@dataclass(frozen=True)
class TuiSettings:
    uiLanguage: str = "en"
    recordLocale: str = ""
    outputDir: str = "output"
    saveRaw: bool = False
    writeDebug: bool = False
    autoPage: bool = True
    rawFile: str = ""
    mapsBuildAssetsRoot: str = ""
    mapsBuildLocale: str = ""
    mapsBuildOutDir: str = ""

    def __post_init__(self) -> None:
        ui_language = self.uiLanguage if self.uiLanguage in UI_LANGUAGES else "en"
        object.__setattr__(self, "uiLanguage", ui_language)
        if not self.recordLocale:
            object.__setattr__(self, "recordLocale", recordLocaleDefaultForUi(ui_language))


def recordLocaleDefaultForUi(uiLanguage: str) -> str:
    if uiLanguage == "zh-Hant":
        return "zh-Hant"
    return DEFAULT_LOCALE


def runtime_settings_dir() -> Path:
    if is_frozen():
        return runtime_root()
    return Path.cwd()


def settings_path(directory: Path | None = None) -> Path:
    return (directory or runtime_settings_dir()) / SETTINGS_FILENAME


def _coerce_bool(value: Any, default: bool) -> bool:
    if isinstance(value, bool):
        return value
    return default


def _coerce_text(value: Any, default: str) -> str:
    return value if isinstance(value, str) and value else default


def settings_from_dict(data: dict[str, Any]) -> TuiSettings:
    defaults = TuiSettings()
    ui_language = _coerce_text(data.get("uiLanguage"), defaults.uiLanguage)
    if ui_language not in UI_LANGUAGES:
        ui_language = defaults.uiLanguage
    record_locale = _coerce_text(data.get("recordLocale"), recordLocaleDefaultForUi(ui_language))
    return TuiSettings(
        uiLanguage=ui_language,
        recordLocale=record_locale,
        outputDir=_coerce_text(data.get("outputDir"), defaults.outputDir),
        saveRaw=_coerce_bool(data.get("saveRaw"), defaults.saveRaw),
        writeDebug=_coerce_bool(data.get("writeDebug"), defaults.writeDebug),
        autoPage=_coerce_bool(data.get("autoPage"), defaults.autoPage),
        rawFile=_coerce_text(data.get("rawFile"), defaults.rawFile),
        mapsBuildAssetsRoot=_coerce_text(data.get("mapsBuildAssetsRoot"), defaults.mapsBuildAssetsRoot),
        mapsBuildLocale=_coerce_text(data.get("mapsBuildLocale"), defaults.mapsBuildLocale),
        mapsBuildOutDir=_coerce_text(data.get("mapsBuildOutDir"), defaults.mapsBuildOutDir),
    )


def load_settings(path: Path | None = None) -> TuiSettings:
    actual_path = path or settings_path()
    try:
        data = json.loads(actual_path.read_text(encoding="utf-8"))
    except (FileNotFoundError, json.JSONDecodeError, OSError):
        return TuiSettings()
    return settings_from_dict(data if isinstance(data, dict) else {})


def _settings_json(settings: TuiSettings) -> str:
    return json.dumps(asdict(settings), ensure_ascii=False, indent=2) + "\n"


def save_settings(settings: TuiSettings, path: Path | None = None, *, allow_admin_relaunch: bool = True) -> None:
    actual_path = path or settings_path()
    try:
        actual_path.parent.mkdir(parents=True, exist_ok=True)
        actual_path.write_text(_settings_json(settings), encoding="utf-8")
    except OSError as exc:
        if allow_admin_relaunch and _request_admin_settings_write(settings):
            raise SettingsAdminRelaunch("administrator settings write requested") from exc
        raise


def import_settings(import_path: Path, path: Path | None = None) -> None:
    settings = load_settings(import_path)
    save_settings(settings, path, allow_admin_relaunch=False)


def _request_admin_settings_write(settings: TuiSettings) -> bool:
    try:
        from nte_gacha_exporter.automation import winapi
    except Exception:
        return False
    if not winapi.is_windows():
        return False
    try:
        if winapi.is_admin():
            return False
    except Exception:
        return False

    temp_dir = Path(tempfile.gettempdir()) / "nte-gacha-exporter"
    temp_dir.mkdir(parents=True, exist_ok=True)
    temp_path = temp_dir / SETTINGS_FILENAME
    temp_path.write_text(_settings_json(settings), encoding="utf-8")

    if is_frozen():
        arguments = ["--import-settings", str(temp_path)]
    else:
        arguments = ["-m", "nte_gacha_exporter.tui.main", "--import-settings", str(temp_path)]
    winapi.relaunch_as_admin(arguments)
    return True
