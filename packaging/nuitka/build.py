from __future__ import annotations

import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[2]
PACKAGING_DIR = PROJECT_ROOT / "packaging" / "nuitka"
TUI_ENTRYPOINT = PACKAGING_DIR / "nte-gacha.py"
CLI_ENTRYPOINT = PACKAGING_DIR / "nte-gacha-cli.py"
WRAPPER_SOURCE = PACKAGING_DIR / "wrapper.c"
OUTPUT_DIR = PROJECT_ROOT / "dist"
APP_NAME = "nte-gacha"
PYPROJECT_FILE = PROJECT_ROOT / "pyproject.toml"
VERSION_PATTERN = re.compile(r'^version\s*=\s*"(?P<version>[^"]+)"\s*$')
APP_VERSION = "0.0.0"
for line in PYPROJECT_FILE.read_text(encoding="utf-8").splitlines():
    match = VERSION_PATTERN.match(line)
    if match:
        APP_VERSION = match.group("version")
        break
RELEASE_DIR = OUTPUT_DIR / f"{APP_NAME}-{APP_VERSION}"
BIN_DIR = RELEASE_DIR / "bin"
RESOURCE_DIR = PROJECT_ROOT / "src" / "nte_gacha_exporter" / "resources"
NUITKA_OUTPUT_FOLDER = "nte-gacha-core"
CORE_EXE_NAME = "nte-gacha-core.exe"
CORE_DIST_DIR = OUTPUT_DIR / f"{NUITKA_OUTPUT_FOLDER}.dist"
WRAPPER_EXE_NAMES = ("nte-gacha.exe", "nte-gacha-cli.exe")
PRESERVED_RELEASE_ROOT_NAMES = {"output"}
RELEASE_ROOT_NAMES = {"bin", "output", "resources", *WRAPPER_EXE_NAMES}


SCAPY_CAPTURE_MODULES = (
    "scapy.arch.common",
    "scapy.arch.libpcap",
    "scapy.arch.windows",
    "scapy.config",
    "scapy.data",
    "scapy.error",
    "scapy.interfaces",
    "scapy.layers.inet",
    "scapy.layers.l2",
    "scapy.libs.winpcapy",
    "scapy.packet",
    "scapy.sendrecv",
    "scapy.supersocket",
    "scapy.utils",
)

SCAPY_UNUSED_IMPORTS = (
    "scapy.contrib",
    "scapy.layers.bluetooth",
    "scapy.layers.dcerpc",
    "scapy.layers.dhcp6",
    "scapy.layers.dns",
    "scapy.layers.dot11",
    "scapy.layers.kerberos",
    "scapy.layers.ldap",
    "scapy.layers.msrpce",
    "scapy.layers.netflow",
    "scapy.layers.smb",
    "scapy.layers.smb2",
    "scapy.layers.tls",
    "scapy.modules.ticketer",
)

PIL_UNUSED_IMPORTS = (
    "PIL.AvifImagePlugin",
    "PIL.BlpImagePlugin",
    "PIL.BmpImagePlugin",
    "PIL.BufrStubImagePlugin",
    "PIL.CurImagePlugin",
    "PIL.DcxImagePlugin",
    "PIL.DdsImagePlugin",
    "PIL.EpsImagePlugin",
    "PIL.FitsImagePlugin",
    "PIL.FliImagePlugin",
    "PIL.FpxImagePlugin",
    "PIL.FtexImagePlugin",
    "PIL.GbrImagePlugin",
    "PIL.GifImagePlugin",
    "PIL.GribStubImagePlugin",
    "PIL.Hdf5StubImagePlugin",
    "PIL.IcnsImagePlugin",
    "PIL.IcoImagePlugin",
    "PIL.ImImagePlugin",
    "PIL.ImageCms",
    "PIL.ImageFilter",
    "PIL.ImageMath",
    "PIL.ImageQt",
    "PIL.ImageShow",
    "PIL.ImageTk",
    "PIL.ImageWin",
    "PIL.ImtImagePlugin",
    "PIL.IptcImagePlugin",
    "PIL.Jpeg2KImagePlugin",
    "PIL.JpegImagePlugin",
    "PIL.JpegPresets",
    "PIL.McIdasImagePlugin",
    "PIL.MicImagePlugin",
    "PIL.MpegImagePlugin",
    "PIL.MpoImagePlugin",
    "PIL.MspImagePlugin",
    "PIL.PalmImagePlugin",
    "PIL.PcdImagePlugin",
    "PIL.PcxImagePlugin",
    "PIL.PdfImagePlugin",
    "PIL.PdfParser",
    "PIL.PixarImagePlugin",
    "PIL.PpmImagePlugin",
    "PIL.PsdImagePlugin",
    "PIL.QoiImagePlugin",
    "PIL.SgiImagePlugin",
    "PIL.SpiderImagePlugin",
    "PIL.SunImagePlugin",
    "PIL.TgaImagePlugin",
    "PIL.TiffImagePlugin",
    "PIL.WebPImagePlugin",
    "PIL.WmfImagePlugin",
    "PIL.XVThumbImagePlugin",
    "PIL.XbmImagePlugin",
    "PIL.XpmImagePlugin",
    "PIL.features",
)

PIL_OCR_MODULES = ("PIL.ImageOps",)

PIL_UNUSED_DLLS = (
    "PIL/_avif.*",
    "PIL/_imagingcms.*",
    "PIL/_imagingmath.*",
    "PIL/_webp.*",
)

WINRT_OCR_MODULES = (
    "winrt.windows.foundation",
    "winrt.windows.foundation.collections",
    "winrt.windows.globalization",
    "winrt.windows.graphics.imaging",
    "winrt.windows.media.ocr",
    "winrt.windows.storage",
    "winrt.windows.storage.streams",
    "winrt.windows.system",
)


def _embeddedPythonPathFile() -> Path:
    return Path(sys.executable).with_name(f"python{sys.version_info.major}{sys.version_info.minor}._pth")


def _nuitkaPythonCommand() -> list[str]:
    command = [sys.executable]

    if _embeddedPythonPathFile().exists():
        command.extend(["-X", "frozen_modules=off", "-S"])

    return command


def _build_command() -> list[str]:
    command = [
        *_nuitkaPythonCommand(),
        "-m",
        "nuitka",
        "--mode=standalone",
        "--mingw64",
        f"--output-dir={OUTPUT_DIR}",
        f"--output-filename={NUITKA_OUTPUT_FOLDER}",
        f"--output-folder-name={NUITKA_OUTPUT_FOLDER}",
        f"--main={TUI_ENTRYPOINT}",
        f"--main={CLI_ENTRYPOINT}",
        "--include-package=nte_gacha_exporter",
        *(f"--include-module={module}" for module in SCAPY_CAPTURE_MODULES),
        *(f"--include-module={module}" for module in PIL_OCR_MODULES),
        *(f"--include-module={module}" for module in WINRT_OCR_MODULES),
        f"--include-data-dir={RESOURCE_DIR}=resources",
        "--noinclude-unittest-mode=nofollow",
        *(f"--noinclude-dlls={pattern}" for pattern in PIL_UNUSED_DLLS),
        *(f"--nofollow-import-to={module}" for module in PIL_UNUSED_IMPORTS),
        "--nofollow-import-to=cv2",
        "--nofollow-import-to=numpy",
        *(f"--nofollow-import-to={module}" for module in SCAPY_UNUSED_IMPORTS),
        "--include-windows-runtime-dlls=yes",
        "--windows-console-mode=force",
        "--assume-yes-for-downloads",
    ]

    if _embeddedPythonPathFile().exists():
        command.insert(command.index("--mode=standalone"), "--must-not-re-execute")

    return command


def _nuitka_mingw_gcc() -> Path | None:
    try:
        from nuitka.utils.Download import getCachedDownloadedMinGW64
        from nuitka.utils.Utils import getArchitecture
    except Exception:
        return None

    try:
        path = getCachedDownloadedMinGW64(
            target_arch=getArchitecture(),
            assume_yes_for_downloads=True,
            download_ok=True,
        )
    except Exception:
        return None
    return Path(path) if path else None


def _find_gcc() -> Path | None:
    for name in ("gcc.exe", "gcc", "x86_64-w64-mingw32-gcc.exe", "x86_64-w64-mingw32-gcc"):
        path = shutil.which(name)
        if path:
            return Path(path)
    return _nuitka_mingw_gcc()


def _wrapper_compile_command(*, compiler: Path, output: Path) -> list[str]:
    return [
        str(compiler),
        "-municode",
        "-O2",
        "-s",
        "-Wall",
        "-Wextra",
        "-o",
        str(output),
        str(WRAPPER_SOURCE),
        "-lshell32",
    ]


def _compile_wrappers(*, environment: dict[str, str], output_dir: Path) -> int:
    compiler = _find_gcc()
    if compiler is None:
        print("C compiler not found. Install gcc or let Nuitka download its MinGW64 toolchain.")
        return 2

    wrapper_env = environment.copy()
    wrapper_env["PATH"] = f"{compiler.parent}{os.pathsep}{wrapper_env.get('PATH', '')}"

    for name in WRAPPER_EXE_NAMES:
        command = _wrapper_compile_command(compiler=compiler, output=output_dir / name)
        code = subprocess.run(command, cwd=PROJECT_ROOT, env=wrapper_env, check=False).returncode
        if code != 0:
            return code
    return 0


def _assert_release_dir_scope() -> None:
    if RELEASE_DIR.exists() and RELEASE_DIR.is_symlink():
        raise RuntimeError(f"refusing to clear symlinked release directory: {RELEASE_DIR}")
    if RELEASE_DIR.parent.resolve() != OUTPUT_DIR.resolve() or RELEASE_DIR.name != f"{APP_NAME}-{APP_VERSION}":
        raise RuntimeError(f"refusing to clear unexpected release directory: {RELEASE_DIR}")


def _assert_build_owned_release_path(path: Path) -> None:
    _assert_release_dir_scope()
    if path.parent != RELEASE_DIR or path.name in PRESERVED_RELEASE_ROOT_NAMES:
        raise RuntimeError(f"refusing to remove path outside release build-owned scope: {path}")


def _build_owned_release_paths() -> tuple[Path, ...]:
    if not RELEASE_DIR.exists():
        return ()
    _assert_release_dir_scope()
    return tuple(path for path in RELEASE_DIR.iterdir() if path.name not in PRESERVED_RELEASE_ROOT_NAMES)


def _clear_build_owned_release_paths() -> None:
    for path in _build_owned_release_paths():
        if not path.exists():
            continue
        _assert_build_owned_release_path(path)
        if path.is_dir() and not path.is_symlink():
            shutil.rmtree(path)
        else:
            path.unlink()


def _stage_release() -> int:
    if not (CORE_DIST_DIR / CORE_EXE_NAME).is_file():
        print(f"Nuitka executable not found after build: {CORE_DIST_DIR / CORE_EXE_NAME}")
        return 2
    for name in WRAPPER_EXE_NAMES:
        if not (CORE_DIST_DIR / name).is_file():
            print(f"Wrapper executable not found after build: {CORE_DIST_DIR / name}")
            return 2

    _clear_build_owned_release_paths()

    RELEASE_DIR.mkdir(parents=True, exist_ok=True)
    for name in WRAPPER_EXE_NAMES:
        shutil.move(str(CORE_DIST_DIR / name), str(RELEASE_DIR / name))
    shutil.move(str(CORE_DIST_DIR), str(BIN_DIR))

    bundled_resources = BIN_DIR / "resources"
    if not bundled_resources.is_dir():
        print(f"Bundled resources not found after build: {bundled_resources}")
        return 2
    shutil.move(str(bundled_resources), str(RELEASE_DIR / "resources"))

    (RELEASE_DIR / "output").mkdir(exist_ok=True)
    return 0


def _validate_release() -> int:
    required_paths = (
        RELEASE_DIR / "nte-gacha.exe",
        RELEASE_DIR / "nte-gacha-cli.exe",
        BIN_DIR / CORE_EXE_NAME,
        RELEASE_DIR / "resources" / "maps",
        RELEASE_DIR / "resources" / "automation",
        RELEASE_DIR / "output",
    )
    missing = [path for path in required_paths if not path.exists()]
    if missing:
        print("Release artifact is incomplete:")
        for path in missing:
            print(f"missing: {path}")
        return 2

    unexpected = sorted(path.name for path in RELEASE_DIR.iterdir() if path.name not in RELEASE_ROOT_NAMES)
    if unexpected:
        print("Release artifact contains unexpected root entries:")
        for name in unexpected:
            print(f"unexpected: {RELEASE_DIR / name}")
        return 2
    return 0


def main() -> int:
    if sys.platform != "win32":
        print("Nuitka packaging is Windows-only. Build on Windows with `poetry install --extras live`.")
        return 2

    environment = os.environ.copy()
    environment["PYTHONHASHSEED"] = "0"

    code = subprocess.run(
        _build_command(),
        cwd=PROJECT_ROOT,
        env=environment,
        check=False,
    ).returncode
    if code != 0:
        return code

    code = _compile_wrappers(environment=environment, output_dir=CORE_DIST_DIR)
    if code != 0:
        return code

    code = _stage_release()
    if code != 0:
        return code

    return _validate_release()


if __name__ == "__main__":
    raise SystemExit(main())
