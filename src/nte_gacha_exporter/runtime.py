from __future__ import annotations

import os
import sys
from pathlib import Path

RUNTIME_ROOT_ENV = "NTE_GACHA_ROOT"
RUNTIME_LAUNCHER_ENV = "NTE_GACHA_LAUNCHER"


def is_frozen() -> bool:
    return bool(getattr(sys, "frozen", False) or globals().get("__compiled__"))


def executable_dir() -> Path:
    return Path(sys.executable).resolve().parent


def runtime_root() -> Path:
    if not is_frozen():
        return Path.cwd()

    env_root = os.environ.get(RUNTIME_ROOT_ENV)
    if env_root:
        return Path(env_root).resolve()

    exe_dir = executable_dir()
    if exe_dir.name.lower() == "bin":
        return exe_dir.parent
    return exe_dir
