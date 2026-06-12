from __future__ import annotations

import json
from importlib import resources
from pathlib import Path
from typing import Any

from nte_gacha_exporter.runtime import is_frozen, runtime_root

RESOURCE_PACKAGE_PREFIX = "nte_gacha_exporter.resources"


def _resource_dir(package: str) -> Path:
    if package == RESOURCE_PACKAGE_PREFIX:
        relative_parts: tuple[str, ...] = ()
    elif package.startswith(f"{RESOURCE_PACKAGE_PREFIX}."):
        relative_parts = tuple(package.removeprefix(f"{RESOURCE_PACKAGE_PREFIX}.").split("."))
    else:
        raise ValueError(f"unsupported resource package: {package}")

    return runtime_root().joinpath("resources", *relative_parts)


def _resource_filename(name: str) -> str:
    if "/" in name or "\\" in name:
        raise ValueError(f"resource name must not include path separators: {name}")
    return f"{name}.json"


def _safe_relative_path(relative_path: str | Path) -> Path:
    path = Path(relative_path)
    if path.is_absolute() or any(part in {"", ".", ".."} for part in path.parts):
        raise ValueError(f"resource path must be a safe relative path: {relative_path}")
    return path


def _resource_file(package: str, name: str) -> Path:
    return _resource_dir(package) / _resource_filename(name)


def resource_path(package: str, relative_path: str | Path) -> Path:
    """Return a filesystem path for a non-JSON resource in the active runtime layout."""

    safe_path = _safe_relative_path(relative_path)
    if is_frozen():
        return _resource_dir(package).joinpath(safe_path)

    resource = resources.files(package).joinpath(*safe_path.parts)
    if not isinstance(resource, Path):
        raise FileNotFoundError(f"resource is not available as a filesystem path: {package}.{relative_path}")
    return resource


def resource_json_path(package: str, name: str) -> Path:
    """Return the filesystem path for a JSON resource in the active runtime layout."""

    if is_frozen():
        return _resource_file(package, name)

    resource = resources.files(package).joinpath(_resource_filename(name))
    if not isinstance(resource, Path):
        raise FileNotFoundError(f"resource is not available as a filesystem path: {package}.{name}")
    return resource


def available_json(package: str) -> list[str]:
    """Return bundled JSON resource names without suffix."""

    if is_frozen():
        directory = _resource_dir(package)
        if not directory.is_dir():
            raise FileNotFoundError(f"resource directory not found: {directory}")
        return sorted(path.name.removesuffix(".json") for path in directory.iterdir() if path.suffix == ".json")

    files = resources.files(package).iterdir()
    return sorted(path.name.removesuffix(".json") for path in files if path.name.endswith(".json"))


def load_json(package: str, name: str) -> Any:
    """Load a bundled JSON resource by name without exposing importlib details."""

    filename = _resource_filename(name)
    try:
        if is_frozen():
            text = _resource_file(package, name).read_text(encoding="utf-8")
        else:
            text = resources.files(package).joinpath(filename).read_text(encoding="utf-8")
    except FileNotFoundError as exc:
        choices = ", ".join(available_json(package)) or "<none>"
        raise FileNotFoundError(f"resource not found: {name}; available: {choices}") from exc
    return json.loads(text)
