from __future__ import annotations

from argparse import Namespace
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

DEFAULT_OUTPUT_DIR = Path("output")
TIMESTAMP_FORMAT = "%y%m%d-%H%M%S"


@dataclass(frozen=True)
class DefaultHistoryPaths:
    timestamp: str
    json: Path
    csv: Path
    raw: Path
    debugJson: Path


def make_timestamp(now: datetime | None = None) -> str:
    return (now or datetime.now()).strftime(TIMESTAMP_FORMAT)


def default_history_paths(
    *,
    output_dir: Path | str = DEFAULT_OUTPUT_DIR,
    timestamp: str | None = None,
) -> DefaultHistoryPaths:
    stamp = timestamp or make_timestamp()
    directory = Path(output_dir)
    return DefaultHistoryPaths(
        timestamp=stamp,
        json=directory / f"history-{stamp}.json",
        csv=directory / f"history-{stamp}.csv",
        raw=directory / f"raw-{stamp}.jsonl",
        debugJson=directory / f"history-debug-{stamp}.json",
    )


def history_default_help(kind: str) -> str:
    examples = {
        "json": "output/history-YYMMDD-HHMMSS.json",
        "csv": "output/history-YYMMDD-HHMMSS.csv",
        "raw": "output/raw-YYMMDD-HHMMSS.jsonl",
        "debug": "output/history-debug-YYMMDD-HHMMSS.json",
    }
    return examples[kind]


def apply_history_output_defaults(args: Namespace, *, timestamp: str | None = None) -> str:
    """Fill timestamped default output paths on a parsed argparse namespace."""

    paths = default_history_paths(timestamp=timestamp)
    if getattr(args, "json", None) in (None, ""):
        args.json = str(paths.json)
    if hasattr(args, "csv") and getattr(args, "csv", None) in (None, ""):
        args.csv = str(paths.csv)
    if hasattr(args, "output_raw") and getattr(args, "output_raw", None) == "":
        args.output_raw = str(paths.raw)
    if hasattr(args, "debug_json") and getattr(args, "debug_json", None) == "":
        args.debug_json = str(paths.debugJson)
    return paths.timestamp
