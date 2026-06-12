"""NTE gacha history exporter."""

from __future__ import annotations

from importlib.metadata import PackageNotFoundError, version

try:
    __version__ = version("nte-gacha-exporter")
except PackageNotFoundError:
    __version__ = "0.1.0"
