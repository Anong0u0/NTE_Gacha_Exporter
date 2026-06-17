from __future__ import annotations

import importlib.util
import runpy
from pathlib import Path
from types import SimpleNamespace

import pytest


def _load_build_module():
    path = Path(__file__).parents[1] / "packaging" / "nuitka" / "build.py"
    spec = importlib.util.spec_from_file_location("nte_nuitka_build", path)
    assert spec is not None
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def test_windows_nuitka_core_build_uses_root_resources_and_pillow_trim(monkeypatch, tmp_path):
    build = _load_build_module()
    captured: dict[str, object] = {"commands": []}

    monkeypatch.setattr(build, "OUTPUT_DIR", tmp_path / "dist")
    monkeypatch.setattr(build, "CORE_RELEASE_DIR", tmp_path / "dist" / "nte-gacha-core-0.1.0")
    monkeypatch.setattr(build, "CORE_RELEASE_BIN_DIR", tmp_path / "dist" / "nte-gacha-core-0.1.0" / "bin")
    monkeypatch.setattr(build, "CORE_DIST_DIR", tmp_path / "dist" / "nte-gacha-core.dist")

    def fake_run(command, *, cwd, env, check):
        captured["commands"].append(command)
        captured["cwd"] = cwd
        captured["env"] = env
        captured["check"] = check
        core_exe = build.CORE_DIST_DIR / "nte-gacha-core.exe"
        maps_dir = build.CORE_DIST_DIR / "resources" / "maps"
        automation_dir = build.CORE_DIST_DIR / "resources" / "automation"
        maps_dir.mkdir(parents=True)
        automation_dir.mkdir(parents=True)
        core_exe.write_text("core", encoding="utf-8")
        for name in build.NUITKA_GENERATED_EXE_NAMES:
            (build.CORE_DIST_DIR / name).write_text("generated", encoding="utf-8")
        return SimpleNamespace(returncode=0)

    monkeypatch.setattr(build.sys, "platform", "win32")
    monkeypatch.setattr(build.subprocess, "run", fake_run)

    assert build.main([]) == 0

    assert build.APP_VERSION == "0.1.0"
    assert build.CORE_RELEASE_DIR.name == "nte-gacha-core-0.1.0"
    commands = captured["commands"]
    assert len(commands) == 1
    command = commands[0]
    assert "--output-filename=nte-gacha-core" in command
    assert "--output-folder-name=nte-gacha-core" in command
    assert f"--main={build.TUI_ENTRYPOINT}" in command
    assert f"--main={build.CLI_ENTRYPOINT}" in command
    assert f"--main={build.SIDECAR_ENTRYPOINT}" not in command
    assert f"--include-data-dir={build.RESOURCE_DIR}=resources" in command
    assert "--include-package-data=nte_gacha_exporter.resources.maps:*.json" not in command
    assert "--include-package-data=nte_gacha_exporter.resources.automation:*.json" not in command
    assert "--noinclude-dlls=PIL/_avif.*" in command
    assert "--noinclude-dlls=PIL/_webp.*" in command
    assert "--nofollow-import-to=PIL.AvifImagePlugin" in command
    assert "--nofollow-import-to=PIL.ImageOps" not in command
    assert all(f"--include-module={module}" in command for module in build.PIL_OCR_MODULES)
    assert "--nofollow-import-to=PIL.PngImagePlugin" not in command
    assert all(f"--include-module={module}" in command for module in build.WINRT_OCR_MODULES)
    assert captured["cwd"] == build.PROJECT_ROOT
    assert captured["check"] is False
    assert (build.CORE_RELEASE_BIN_DIR / "nte-gacha-core.exe").read_text(encoding="utf-8") == "core"
    assert not (build.CORE_RELEASE_DIR / "nte-gacha-python-core.exe").exists()
    assert not (build.CORE_RELEASE_BIN_DIR / "nte-gacha.exe").exists()
    assert not (build.CORE_RELEASE_BIN_DIR / "nte-gacha-cli.exe").exists()
    assert (build.CORE_RELEASE_DIR / "resources" / "maps").is_dir()
    assert (build.CORE_RELEASE_DIR / "resources" / "automation").is_dir()
    assert not hasattr(build, "CLI_DIST_DIR")


def test_windows_nuitka_sidecar_target_stages_sidecar_layout(monkeypatch, tmp_path):
    build = _load_build_module()
    captured: dict[str, object] = {"commands": []}

    monkeypatch.setattr(build, "OUTPUT_DIR", tmp_path / "dist")
    monkeypatch.setattr(build, "SIDECAR_RELEASE_DIR", tmp_path / "dist" / "nte-gacha-sidecar-0.1.0")
    monkeypatch.setattr(
        build,
        "SIDECAR_RELEASE_BIN_DIR",
        tmp_path / "dist" / "nte-gacha-sidecar-0.1.0" / "bin",
    )
    monkeypatch.setattr(build, "CORE_DIST_DIR", tmp_path / "dist" / "nte-gacha-core.dist")
    fake_gcc = tmp_path / "gcc.exe"
    fake_gcc.write_text("gcc", encoding="utf-8")
    monkeypatch.setattr(build, "_find_gcc", lambda: fake_gcc)

    def fake_run(command, *, cwd, env, check):
        captured["commands"].append(command)
        if "-m" in command and "nuitka" in command:
            core_exe = build.CORE_DIST_DIR / "nte-gacha-core.exe"
            maps_dir = build.CORE_DIST_DIR / "resources" / "maps"
            automation_dir = build.CORE_DIST_DIR / "resources" / "automation"
            maps_dir.mkdir(parents=True)
            automation_dir.mkdir(parents=True)
            core_exe.write_text("core", encoding="utf-8")
            (build.CORE_DIST_DIR / "nte-gacha-sidecar.exe").write_text("generated", encoding="utf-8")
        else:
            out = Path(command[command.index("-o") + 1])
            out.write_text("wrapper", encoding="utf-8")
        return SimpleNamespace(returncode=0)

    monkeypatch.setattr(build.sys, "platform", "win32")
    monkeypatch.setattr(build.subprocess, "run", fake_run)

    assert build.main(["--target", "sidecar"]) == 0

    commands = captured["commands"]
    assert len(commands) == 2
    command = commands[0]
    assert f"--main={build.SIDECAR_ENTRYPOINT}" in command
    assert f"--main={build.TUI_ENTRYPOINT}" not in command
    assert f"--main={build.CLI_ENTRYPOINT}" not in command
    wrapper_command = commands[1]
    assert "-municode" in wrapper_command
    assert str(build.WRAPPER_SOURCE) in wrapper_command
    assert (build.SIDECAR_RELEASE_DIR / "nte-gacha-python-core.exe").read_text(encoding="utf-8") == "wrapper"
    assert (build.SIDECAR_RELEASE_BIN_DIR / "nte-gacha-core.exe").read_text(encoding="utf-8") == "core"
    assert not (build.SIDECAR_RELEASE_BIN_DIR / "nte-gacha-sidecar.exe").exists()
    assert (build.SIDECAR_RELEASE_DIR / "resources" / "maps").is_dir()
    assert (build.SIDECAR_RELEASE_DIR / "resources" / "automation").is_dir()


def test_stage_core_release_clears_existing_build_owned_paths(monkeypatch, tmp_path):
    build = _load_build_module()

    monkeypatch.setattr(build, "OUTPUT_DIR", tmp_path / "dist")
    monkeypatch.setattr(build, "CORE_RELEASE_DIR", tmp_path / "dist" / "nte-gacha-core-0.1.0")
    monkeypatch.setattr(build, "CORE_RELEASE_BIN_DIR", tmp_path / "dist" / "nte-gacha-core-0.1.0" / "bin")
    monkeypatch.setattr(build, "CORE_DIST_DIR", tmp_path / "dist" / "nte-gacha-core.dist")
    core_exe = build.CORE_DIST_DIR / build.CORE_EXE_NAME
    core_exe.parent.mkdir(parents=True)
    core_exe.write_text("core", encoding="utf-8")
    for name in build.NUITKA_GENERATED_EXE_NAMES:
        (build.CORE_DIST_DIR / name).write_text("generated", encoding="utf-8")
    maps_dir = build.CORE_DIST_DIR / "resources" / "maps"
    automation_dir = build.CORE_DIST_DIR / "resources" / "automation"
    maps_dir.mkdir(parents=True)
    automation_dir.mkdir(parents=True)
    build.CORE_RELEASE_BIN_DIR.mkdir(parents=True)
    (build.CORE_RELEASE_BIN_DIR / "old.exe").write_text("old", encoding="utf-8")
    (build.CORE_RELEASE_DIR / "resources" / "stale.json").parent.mkdir()
    (build.CORE_RELEASE_DIR / "resources" / "stale.json").write_text("old", encoding="utf-8")
    (build.CORE_RELEASE_DIR / "debug.txt").write_text("old", encoding="utf-8")

    assert build._stage_release() == 0

    assert (build.CORE_RELEASE_BIN_DIR / "nte-gacha-core.exe").read_text(encoding="utf-8") == "core"
    assert not (build.CORE_RELEASE_BIN_DIR / "old.exe").exists()
    assert not (build.CORE_RELEASE_BIN_DIR / "nte-gacha.exe").exists()
    assert not (build.CORE_RELEASE_DIR / "resources" / "stale.json").exists()
    assert not (build.CORE_RELEASE_DIR / "debug.txt").exists()
    assert not list((tmp_path / "dist").glob("nte-gacha.previous-*"))


def test_clear_build_owned_release_paths_rejects_unscoped_paths(monkeypatch, tmp_path):
    build = _load_build_module()

    monkeypatch.setattr(build, "OUTPUT_DIR", tmp_path / "dist")
    monkeypatch.setattr(build, "CORE_RELEASE_DIR", tmp_path / "dist" / "nte-gacha-core-0.1.0")

    with pytest.raises(RuntimeError, match="outside release build-owned scope"):
        build._assert_build_owned_release_path(
            build.CORE_RELEASE_DIR / "nested" / "file.txt",
            release_dir=build.CORE_RELEASE_DIR,
            target=build.CORE_TARGET,
        )


def test_clear_build_owned_release_paths_rejects_unexpected_release_dir(monkeypatch, tmp_path):
    build = _load_build_module()

    monkeypatch.setattr(build, "OUTPUT_DIR", tmp_path / "dist")
    monkeypatch.setattr(build, "CORE_RELEASE_DIR", tmp_path / "dist" / "nte-gacha")
    build.CORE_RELEASE_DIR.mkdir(parents=True)

    with pytest.raises(RuntimeError, match="unexpected release directory"):
        build._build_owned_release_paths(
            release_dir=build.CORE_RELEASE_DIR,
            target=build.CORE_TARGET,
        )


def test_validate_release_rejects_unexpected_root_entries(monkeypatch, tmp_path, capsys):
    build = _load_build_module()

    monkeypatch.setattr(build, "SIDECAR_RELEASE_DIR", tmp_path / "dist" / "nte-gacha-sidecar-0.1.0")
    monkeypatch.setattr(
        build,
        "SIDECAR_RELEASE_BIN_DIR",
        tmp_path / "dist" / "nte-gacha-sidecar-0.1.0" / "bin",
    )
    for path in (
        build.SIDECAR_RELEASE_DIR / "nte-gacha-python-core.exe",
        build.SIDECAR_RELEASE_BIN_DIR / build.CORE_EXE_NAME,
        build.SIDECAR_RELEASE_DIR / "resources" / "maps",
        build.SIDECAR_RELEASE_DIR / "resources" / "automation",
        build.SIDECAR_RELEASE_DIR / "debug.txt",
    ):
        path.parent.mkdir(parents=True, exist_ok=True)
        if "." in path.name:
            path.write_text("", encoding="utf-8")
        else:
            path.mkdir(exist_ok=True)

    assert build._validate_release(build.SIDECAR_TARGET) == 2
    assert "unexpected" in capsys.readouterr().out


def test_pyproject_declares_winrt_ocr_dependencies():
    pyproject = (Path(__file__).parents[1] / "pyproject.toml").read_text(encoding="utf-8")

    for package in (
        "winrt-Windows.Foundation",
        "winrt-Windows.Foundation.Collections",
        "winrt-Windows.Globalization",
        "winrt-Windows.Graphics.Imaging",
        "winrt-Windows.Media.Ocr",
        "winrt-Windows.Storage",
        "winrt-Windows.Storage.Streams",
        "winrt-Windows.System",
    ):
        assert f'"{package}" = {{ version = ">=3.2,<4.0", markers = "sys_platform == \'win32\'" }}' in pyproject


def test_nuitka_cli_entrypoint_calls_cli_main(monkeypatch):
    seen: dict[str, object] = {}

    def fake_main():
        seen["called"] = True
        return 7

    monkeypatch.setattr("nte_gacha_exporter.cli.main.main", fake_main)

    path = Path(__file__).parents[1] / "packaging" / "nuitka" / "nte-gacha-cli.py"
    with pytest.raises(SystemExit) as exc:
        runpy.run_path(str(path), run_name="__main__")

    assert exc.value.code == 7
    assert seen == {"called": True}


def test_nuitka_sidecar_entrypoint_calls_sidecar_main(monkeypatch):
    seen: dict[str, object] = {}

    def fake_main():
        seen["called"] = True
        return 0

    monkeypatch.setattr("nte_gacha_exporter.sidecar.main.main", fake_main)

    path = Path(__file__).parents[1] / "packaging" / "nuitka" / "nte-gacha-sidecar.py"
    with pytest.raises(SystemExit) as exc:
        runpy.run_path(str(path), run_name="__main__")

    assert exc.value.code == 0
    assert seen == {"called": True}
