from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from nte_gacha_exporter import runtime as app_runtime
from nte_gacha_exporter.capture.live import CaptureEnvironmentError, doctor, list_interfaces
from nte_gacha_exporter.core.schema import ExportDocument
from nte_gacha_exporter.export.pipeline import export_capture
from nte_gacha_exporter.export.writers import write_csv, write_debug_json, write_json
from nte_gacha_exporter.mapping.assets import build_map, find_assets_root
from nte_gacha_exporter.mapping.runtime import available_locales

REMOVED_MAP_LOCALES = {"en-JM"}


@dataclass(frozen=True)
class OperationResult:
    exitCode: int
    lines: tuple[str, ...] = ()
    document: ExportDocument | None = None
    paths: dict[str, Path] = field(default_factory=dict)
    error: str | None = None
    captureCounts: str | None = None
    lastRecords: tuple[dict[str, Any], ...] = ()


def is_frozen() -> bool:
    return app_runtime.is_frozen()


def executable_dir() -> Path:
    return app_runtime.executable_dir()


def default_maps_output_dir() -> Path:
    if is_frozen():
        return app_runtime.runtime_root() / "resources" / "maps"
    return Path("src/nte_gacha_exporter/resources/maps")


def write_history_outputs(
    *,
    json_out: Path,
    csv_out: Path | None,
    debug_json_out: Path | None = None,
    document: ExportDocument,
) -> None:
    write_json(json_out, document)
    if debug_json_out:
        write_debug_json(debug_json_out, document)
    if csv_out:
        write_csv(csv_out, document)


def run_debug_export(
    *,
    raw_jsonl: Path,
    locale: str,
    json_out: Path,
    csv_out: Path | None,
    debug_json_out: Path | None = None,
) -> OperationResult:
    try:
        document = export_capture(raw_jsonl, locale=locale)
        write_history_outputs(
            json_out=json_out,
            csv_out=csv_out,
            debug_json_out=debug_json_out,
            document=document,
        )
    except Exception as exc:
        return OperationResult(2, error=f"export failed: {exc}")

    paths = {"json": json_out}
    if debug_json_out:
        paths["debug_json"] = debug_json_out
    if csv_out:
        paths["csv"] = csv_out
    return OperationResult(0, document=document, paths=paths)


def run_doctor() -> OperationResult:
    code, lines = doctor()
    return OperationResult(code, lines=tuple(lines))


def run_interfaces() -> OperationResult:
    try:
        return OperationResult(0, lines=tuple(list_interfaces()))
    except CaptureEnvironmentError as exc:
        return OperationResult(3, error=str(exc))


def run_maps_list() -> OperationResult:
    return OperationResult(0, lines=tuple(available_locales()))


def run_maps_build(
    *,
    assets_root: str | None,
    locale: str | None,
    out_dir: Path | None = None,
) -> OperationResult:
    try:
        actual_assets_root = find_assets_root(assets_root)
        actual_out_dir = out_dir or default_maps_output_dir()
        actual_out_dir.mkdir(parents=True, exist_ok=True)

        locales = (
            [locale]
            if locale
            else sorted(
                path.name
                for path in (actual_assets_root / "Localization").iterdir()
                if path.is_dir() and (path / "game.json").exists()
            )
        )
        locales = [item for item in locales if item not in REMOVED_MAP_LOCALES]
        lines: list[str] = []
        for item in locales:
            map_data = build_map(actual_assets_root, item)
            out = actual_out_dir / f"{item}.json"
            out.write_text(json.dumps(map_data, ensure_ascii=False, indent=2), encoding="utf-8")
            lines.append(
                f"{item}: items={len(map_data['items'])} pools={len(map_data['pools'])} "
                f"labels={len(map_data['labels'])}"
            )
        return OperationResult(0, lines=tuple(lines), paths={"out_dir": actual_out_dir})
    except Exception as exc:
        return OperationResult(2, error=f"maps build failed: {exc}")


def result_path_lines(paths: dict[str, Path]) -> tuple[str, ...]:
    ordered_keys = ("json", "debug_json", "csv", "private_raw", "out_dir")
    return tuple(f"{key}={paths[key]}" for key in ordered_keys if key in paths)
