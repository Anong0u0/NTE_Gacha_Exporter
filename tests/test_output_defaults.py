from __future__ import annotations

from argparse import Namespace
from datetime import datetime
from pathlib import Path

from nte_gacha_exporter.app.output import apply_history_output_defaults, default_history_paths, make_timestamp


def test_make_timestamp_uses_yymmdd_hhmmss():
    assert make_timestamp(datetime(2026, 6, 11, 15, 30, 12)) == "260611-153012"


def test_default_history_paths_share_one_timestamp():
    paths = default_history_paths(timestamp="260611-153012")

    assert paths.json == Path("output/history-260611-153012.json")
    assert paths.csv == Path("output/history-260611-153012.csv")
    assert paths.raw == Path("output/raw-260611-153012.jsonl")
    assert paths.debugJson == Path("output/history-debug-260611-153012.json")


def test_apply_history_output_defaults_preserves_explicit_paths():
    args = Namespace(
        json="custom/history.json",
        csv=None,
        output_raw="custom/raw.jsonl",
        debug_json="",
    )

    apply_history_output_defaults(args, timestamp="260611-153012")

    assert args.json == "custom/history.json"
    assert args.csv == "output/history-260611-153012.csv"
    assert args.output_raw == "custom/raw.jsonl"
    assert args.debug_json == "output/history-debug-260611-153012.json"
