from __future__ import annotations

import json
from pathlib import Path
from typing import cast

from nte_gacha_exporter.core.schema import LocalizationMap
from nte_gacha_exporter.resources.json_data import available_json, load_json

DEFAULT_LOCALE = "en"
MAP_PACKAGE = "nte_gacha_exporter.resources.maps"


def available_locales() -> list[str]:
    """Return bundled localization map names."""

    return available_json(MAP_PACKAGE)


def _validate_map(data: object, *, source: str) -> LocalizationMap:
    if not isinstance(data, dict):
        raise ValueError(f"localization map must be an object: {source}")

    for key in ("csv_headers", "items", "pools", "pool_meta", "labels"):
        value = data.get(key, {})
        if value is not None and not isinstance(value, dict):
            raise ValueError(f"localization map section must be an object: {source}:{key}")

    return cast(LocalizationMap, data)


def load_map(locale: str = DEFAULT_LOCALE) -> LocalizationMap:
    """Load a bundled localization map."""

    try:
        data = load_json(MAP_PACKAGE, locale)
    except FileNotFoundError as exc:
        choices = ", ".join(available_locales()) or "<none>"
        raise FileNotFoundError(f"locale not found: {locale}; available: {choices}") from exc
    return _validate_map(data, source=f"{locale}.json")


def load_map_file(path: str | Path) -> LocalizationMap:
    """Load a user-provided localization map file."""

    map_path = Path(path)
    return _validate_map(json.loads(map_path.read_text(encoding="utf-8")), source=str(map_path))


def load_locale_map(locale_spec: str = DEFAULT_LOCALE) -> tuple[str, LocalizationMap]:
    """Load a bundled locale or a user-provided map from 'locale=path.json'."""

    if "=" not in locale_spec:
        return locale_spec, load_map(locale_spec)

    locale, path = locale_spec.split("=", 1)
    if not locale or not path:
        raise ValueError("custom locale map must use locale=path.json")
    return locale, load_map_file(path)
