from __future__ import annotations

import json
import sys

import pytest

from nte_gacha_exporter.app.operations import default_maps_output_dir
from nte_gacha_exporter.resources.json_data import available_json, load_json
from nte_gacha_exporter.runtime import runtime_root
from nte_gacha_exporter.tui.settings import settings_path


def test_frozen_resources_are_loaded_from_release_root_env(monkeypatch, tmp_path):
    release_root = tmp_path / "nte-gacha"
    exe_path = release_root / "bin" / "nte-gacha-core.exe"
    maps_dir = release_root / "resources" / "maps"
    maps_dir.mkdir(parents=True)
    (maps_dir / "custom.json").write_text(json.dumps({"ok": True}), encoding="utf-8")

    monkeypatch.setattr(sys, "frozen", True, raising=False)
    monkeypatch.setattr(sys, "executable", str(exe_path))
    monkeypatch.setenv("NTE_GACHA_ROOT", str(release_root))

    assert available_json("nte_gacha_exporter.resources.maps") == ["custom"]
    assert load_json("nte_gacha_exporter.resources.maps", "custom") == {"ok": True}
    assert runtime_root() == release_root


def test_frozen_resources_do_not_fallback_to_package_data(monkeypatch, tmp_path):
    release_root = tmp_path / "nte-gacha"
    exe_path = release_root / "bin" / "nte-gacha-core.exe"
    exe_path.parent.mkdir(parents=True)

    monkeypatch.setattr(sys, "frozen", True, raising=False)
    monkeypatch.setattr(sys, "executable", str(exe_path))

    with pytest.raises(FileNotFoundError, match="resource directory not found"):
        available_json("nte_gacha_exporter.resources.maps")


def test_frozen_resource_names_reject_path_separators(monkeypatch, tmp_path):
    release_root = tmp_path / "nte-gacha"
    exe_path = release_root / "bin" / "nte-gacha-core.exe"
    (release_root / "resources" / "maps").mkdir(parents=True)

    monkeypatch.setattr(sys, "frozen", True, raising=False)
    monkeypatch.setattr(sys, "executable", str(exe_path))

    with pytest.raises(ValueError, match="path separators"):
        load_json("nte_gacha_exporter.resources.maps", "../zh-Hant")


def test_frozen_runtime_root_falls_back_from_bin_executable(monkeypatch, tmp_path):
    release_root = tmp_path / "nte-gacha"
    exe_path = release_root / "bin" / "nte-gacha-core.exe"
    exe_path.parent.mkdir(parents=True)

    monkeypatch.setattr(sys, "frozen", True, raising=False)
    monkeypatch.setattr(sys, "executable", str(exe_path))
    monkeypatch.delenv("NTE_GACHA_ROOT", raising=False)

    assert runtime_root() == release_root


def test_packaged_defaults_use_release_root(monkeypatch, tmp_path):
    release_root = tmp_path / "nte-gacha"
    exe_path = release_root / "bin" / "nte-gacha-core.exe"
    exe_path.parent.mkdir(parents=True)

    monkeypatch.setattr(sys, "frozen", True, raising=False)
    monkeypatch.setattr(sys, "executable", str(exe_path))
    monkeypatch.setenv("NTE_GACHA_ROOT", str(release_root))

    assert default_maps_output_dir() == release_root / "resources" / "maps"
    assert settings_path() == release_root / "tui-settings.json"
