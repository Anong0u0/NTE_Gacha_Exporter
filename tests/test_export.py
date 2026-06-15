from __future__ import annotations

import csv
import json
from dataclasses import replace
from datetime import timedelta, timezone
from pathlib import Path

from nte_gacha_exporter.core.models import ParsedBlock, ParsedRow, ProtocolEnvelope, SourceRef
from nte_gacha_exporter.export.assembler import ProtocolAssembler
from nte_gacha_exporter.export.document import ExportOptions, _pool_name, build_document, public_records
from nte_gacha_exporter.export.pipeline import export_capture
from nte_gacha_exporter.export.writers import write_csv, write_debug_json, write_json

FIXTURE = Path(__file__).parent / "fixtures" / "sample.raw.jsonl"


def test_export_document_is_sanitized_and_localized():
    document = export_capture(FIXTURE, locale="zh-Hant")

    assert document["info"]["schema"] == "nte-gacha-export"
    assert document["info"]["export_app"] == "nte-gacha-exporter"
    assert isinstance(document["info"]["export_timestamp"], int)
    assert document["info"]["name_source"] == "localization_map"
    assert document["info"]["time_source"] == "decoded_dotnet_ticks"
    assert document["info"]["privacy"] == "sanitized"
    assert document["_debug"]["summary"]["record_count"] == 2
    assert document["nte"]["list"][0]["time"] == "2026-04-30 17:02:15"
    assert document["nte"]["list"][0]["pool_name"] == "王牌一代目"
    assert document["nte"]["list"][0]["item_name"] == "改裝件·萌虎來襲-塗裝"
    assert "secondary_item_id" not in document["nte"]["list"][0]
    assert "secondary_item_name" not in document["nte"]["list"][0]
    assert "secondary_count" not in document["nte"]["list"][0]
    assert "secondary_item_id" not in document["nte"]["list"][1]
    assert document["nte"]["list"][1]["pool_name"] == "夜曲特刊"
    assert document["nte"]["list"][1]["item_name"] == "弧盤·危險遊戲"
    text = json.dumps(document, ensure_ascii=False)
    assert "src" not in text
    assert "dst" not in text
    assert "payload_b64" not in text


def test_export_capture_silently_ignores_duplicate_records(tmp_path):
    lines = FIXTURE.read_text(encoding="utf-8").splitlines()
    packet_lines = [line for line in lines if json.loads(line).get("type") == "packet"]
    raw_path = tmp_path / "duplicate.raw.jsonl"
    raw_path.write_text("\n".join([lines[0], *packet_lines, *packet_lines, lines[-1]]) + "\n", encoding="utf-8")

    document = export_capture(raw_path, locale="zh-Hant")

    records = document["nte"]["list"]
    assert document["_debug"]["summary"]["record_count"] == 2
    assert document["_debug"]["summary"]["warning_count"] == 0
    assert len(document["_debug"]["records"]) == 2
    assert len(document["_debug"]["raw_rows"]) == 2
    assert len({record["record_id"] for record in records}) == 2
    assert all(len(record["record_id"]) == 64 for record in records)
    assert all(set(record["record_id"]) <= set("0123456789abcdef") for record in records)


def test_json_writer_omits_debug_data(tmp_path):
    document = export_capture(FIXTURE, locale="zh-Hant")
    out = tmp_path / "history.json"

    write_json(out, document)

    data = json.loads(out.read_text(encoding="utf-8"))
    assert sorted(data) == ["info", "nte"]
    assert "_debug" not in data
    assert "_meta" not in data
    assert "csv_headers" not in json.dumps(data, ensure_ascii=False)


def test_debug_json_writer_has_diagnostics(tmp_path):
    document = export_capture(FIXTURE, locale="zh-Hant")
    out = tmp_path / "history.debug.json"

    write_debug_json(out, document)

    data = json.loads(out.read_text(encoding="utf-8"))
    assert data["summary"]["record_count"] == 2
    assert data["raw_rows"][0]["ticks"] == 639131653353040000
    assert data["records"][0]["source"] == {"packet_index": 0, "row_index": 0, "offset": 39}
    assert data["raw_rows"][0]["source"] == {"packet_index": 0, "row_index": 0, "offset": 39}
    assert "secondary_item_id" not in data["records"][0]
    assert "secondary_item_name" not in data["records"][0]
    assert "secondary_count" not in data["records"][0]
    assert data["raw_rows"][0]["secondary_item_id"] == "Fashion_vehicle_1010_V008"
    text = json.dumps(data, ensure_ascii=False)
    assert "item_spec" not in text
    assert '"session"' not in text
    assert '"line"' not in text
    assert '"view"' not in text
    assert '"row_index"' in text
    assert '"offset"' in text


def test_csv_writer_has_stable_human_header(tmp_path):
    document = export_capture(FIXTURE, locale="zh-Hant")
    out = tmp_path / "history.csv"

    write_csv(out, document)

    assert not out.read_bytes().startswith(b"\xef\xbb\xbf")
    with out.open("r", encoding="utf-8", newline="") as fh:
        reader = csv.DictReader(fh)
        rows = list(reader)

    assert reader.fieldnames == [
        "獲得時間",
        "卡池類型",
        "卡池",
        "道具名稱",
        "數量",
        "投擲點數",
        "額外獲得",
        "額外獲得數量",
    ]
    assert rows[0]["獲得時間"] == "2026-04-30 17:02:15"
    assert rows[0]["卡池類型"] == "限定棋盤"
    assert rows[0]["卡池"] == "王牌一代目"
    assert rows[0]["道具名稱"] == "改裝件·萌虎來襲-塗裝"
    assert rows[0]["數量"] == "1"
    assert rows[0]["投擲點數"] == "2"
    assert rows[0]["額外獲得"] == ""
    assert rows[0]["額外獲得數量"] == ""
    assert rows[1]["卡池類型"] == "弧盤研募"
    assert rows[1]["卡池"] == "夜曲特刊"
    assert rows[1]["額外獲得"] == ""


def test_locale_can_point_to_custom_map_file(tmp_path):
    custom_map = {
        "schema_version": 2,
        "csv_headers": {},
        "items": {
            "Fashion_vehicle_1010_V008": {"name": "自訂名稱", "rarity": 5, "category": "vehicle_module"},
            "fork_dustbin": {"name": "弧盤·危險遊戲", "rarity": 4, "category": "fork"},
        },
        "item_aliases": {},
        "pools": {
            "CardPool_Character": {"name": "自訂限定", "group_label": "自訂限定", "title": "自訂池"},
            "ForkLottery_AnHunQu": {"name": "奇蹟盒盒", "group_label": "弧盤研募", "title": "夜曲特刊"},
        },
        "labels": {},
    }
    map_path = tmp_path / "custom.json"
    map_path.write_text(json.dumps(custom_map, ensure_ascii=False), encoding="utf-8")

    document = export_capture(FIXTURE, locale=f"custom={map_path}")

    assert document["info"]["locale"] == "custom"
    assert document["nte"]["list"][0]["pool_name"] == "自訂池"
    assert document["nte"]["list"][0]["item_name"] == "自訂名稱"


def test_monopoly_pool_name_uses_tz8_windows_as_host_local_native_time():
    mapping = {
        "pools": {"CardPool_Character": "限定棋盤"},
        "pool_meta": {
            "CardPool_Character": {
                "title_windows": [
                    {"end_at_tz8": "2026-05-13 05:59:00", "title": "王牌一代目"},
                    {"end_at_tz8": "2026-06-03 05:59:00", "title": "獨酌朧月流"},
                    {"end_at_tz8": "2026-06-24 05:59:00", "title": "久夢初醒時"},
                    {"end_at_tz8": "2026-07-08 05:59:00", "title": "無歸路"},
                ]
            }
        },
    }
    tz8 = timezone(timedelta(hours=8))
    utc = timezone.utc

    assert _pool_name(mapping, "CardPool_Character", "2026-05-13 05:59:00", local_tz=tz8) == "王牌一代目"
    assert _pool_name(mapping, "CardPool_Character", "2026-05-13 05:59:01", local_tz=tz8) == "獨酌朧月流"
    assert _pool_name(mapping, "CardPool_Character", "2026-05-12 21:59:00", local_tz=utc) == "王牌一代目"
    assert _pool_name(mapping, "CardPool_Character", "2026-05-12 21:59:01", local_tz=utc) == "獨酌朧月流"
    assert _pool_name(mapping, "CardPool_Character", "2026-07-08 05:59:01", local_tz=tz8) == "限定棋盤"


def test_public_record_keeps_secondary_fields_when_item_is_different():
    source = SourceRef(session=0, line=1, packet_index=0, view="src", row_index=0, offset=0)
    row = ParsedRow(
        record_type="monopoly",
        ticks=639164696613410000,
        time="2026-06-07T22:14:21.341000",
        pool_id="CardPool_NewRole",
        item_id="fork_nonos",
        count=1,
        roll_points=5,
        roll_label_id=None,
        secondary_item_id="Dice_ticket_01",
        secondary_count=2,
        source=source,
    )
    mapping = {
        "pools": {"CardPool_NewRole": "標準棋盤"},
        "items": {"fork_nonos": "弧盤·霓歐斯", "Dice_ticket_01": "道具·質實骰子"},
    }

    record = public_records([row], mapping)[0].to_dict()

    assert record["secondary_item_id"] == "Dice_ticket_01"
    assert record["secondary_item_name"] == "道具·質實骰子"
    assert record["secondary_count"] == 2


def test_public_record_canonicalizes_item_aliases_before_localizing_and_deduping():
    source = SourceRef(session=0, line=1, packet_index=0, view="src", row_index=0, offset=0)
    row = ParsedRow(
        record_type="monopoly",
        ticks=639164696613410000,
        time="2026-06-07T22:14:21.341000",
        pool_id="CardPool_NewRole",
        item_id="DIceNormal",
        count=1,
        roll_points=5,
        roll_label_id=None,
        secondary_item_id="DIceLimite",
        secondary_count=2,
        source=source,
    )
    mapping = {
        "pools": {"CardPool_NewRole": "標準棋盤"},
        "items": {"DiceNormal": "道具·捏造骰子", "Dicelimite": "道具·質實骰子"},
        "item_aliases": {"DIceNormal": "DiceNormal", "DIceLimite": "Dicelimite"},
    }

    document = build_document([row], mapping, ExportOptions(locale="zh-Hant", source="test"))
    record = document["nte"]["list"][0]

    assert record["item_id"] == "DiceNormal"
    assert record["item_name"] == "道具·捏造骰子"
    assert record["secondary_item_id"] == "Dicelimite"
    assert record["secondary_item_name"] == "道具·質實骰子"
    assert "DIceNormal" not in record["record_id"]
    assert "DIceLimite" not in record["record_id"]
    assert document["_debug"]["summary"]["warning_count"] == 0


def test_build_document_keeps_matching_public_rows_from_distinct_sources():
    source = SourceRef(session=0, line=1, packet_index=0, view="src", row_index=0, offset=0)
    row = ParsedRow(
        record_type="monopoly",
        ticks=639164696613410000,
        time="2026-06-07T22:14:21.341000",
        pool_id="CardPool_NewRole",
        item_id="fork_nonos",
        count=1,
        roll_points=5,
        roll_label_id=None,
        secondary_item_id="Dice_ticket_01",
        secondary_count=1,
        source=source,
    )
    document = build_document(
        [row, replace(row, source=SourceRef(session=0, line=2, packet_index=1, view="src", row_index=0, offset=0))],
        {
            "pools": {"CardPool_NewRole": "標準棋盤"},
            "items": {"fork_nonos": "弧盤·霓歐斯", "Dice_ticket_01": "道具·質實骰子"},
        },
        ExportOptions(locale="zh-Hant", source="test"),
    )

    records = document["nte"]["list"]
    assert document["_debug"]["summary"]["record_count"] == 2
    assert [record["secondary_count"] for record in records] == [1, 1]


def _row_for_segment(index: int, *, segment_index: int = 0) -> ParsedRow:
    return ParsedRow(
        record_type="fork",
        ticks=639000000000000000 + index,
        time=f"2026-06-10T08:00:{index:02d}.000000",
        pool_id="ForkLottery_Test",
        item_id=f"fork_{index}",
        count=1,
        roll_points=None,
        roll_label_id=None,
        secondary_item_id=None,
        secondary_count=None,
        source=SourceRef(
            session=0,
            line=index,
            packet_index=index,
            view="shift8:1",
            row_index=index,
            offset=index,
            stream_key="fork",
            page_index=(segment_index + 1) // 2,
            query_high=segment_index % 2 == 0,
            segment_index=segment_index,
        ),
    )


def _block(segment_index: int, row_indexes: list[int], *, stream_key: str = "fork") -> ParsedBlock:
    rows = tuple(_row_for_segment(index, segment_index=segment_index) for index in row_indexes)
    envelope = ProtocolEnvelope(
        record_type="fork",
        stream_key=stream_key,
        page_index=(segment_index + 1) // 2,
        query_high=segment_index % 2 == 0,
        segment_index=segment_index,
    )
    return ParsedBlock("fork", 33, 0, len(rows), rows, envelope)


def test_protocol_assembler_ignores_exact_segment_replay():
    assembler = ProtocolAssembler()

    first = assembler.add_blocks([_block(0, [0, 1, 2, 3, 4])])
    second = assembler.add_blocks([_block(0, [0, 1, 2, 3, 4])])

    assert [row.item_id for row in first] == [f"fork_{index}" for index in range(5)]
    assert second == []
    assert [row.item_id for row in assembler.rows()] == [f"fork_{index}" for index in range(5)]


def test_protocol_assembler_accepts_late_missing_segment_fill():
    assembler = ProtocolAssembler()

    assembler.add_blocks([_block(0, [0, 1, 2, 3, 4]), _block(2, [10, 11, 12, 13, 14])])
    assembler.add_blocks([_block(1, [5, 6, 7, 8, 9])])

    assert [row.item_id for row in assembler.rows()] == [f"fork_{index}" for index in range(15)]
    assert assembler.warnings == []


def test_protocol_assembler_replaces_full_recapture_after_shift():
    assembler = ProtocolAssembler()

    assembler.add_blocks([_block(0, [0, 1, 2, 3, 4]), _block(1, [5, 6, 7, 8, 9])])
    assembler.add_blocks(
        [
            _block(0, [99, 0, 1, 2, 3]),
            _block(1, [4, 5, 6, 7, 8]),
            _block(2, [9]),
        ]
    )

    assert [row.item_id for row in assembler.rows()] == [
        "fork_99",
        "fork_0",
        "fork_1",
        "fork_2",
        "fork_3",
        "fork_4",
        "fork_5",
        "fork_6",
        "fork_7",
        "fork_8",
        "fork_9",
    ]


def test_protocol_assembler_merges_partial_recapture_only_when_alignment_is_unique():
    assembler = ProtocolAssembler()

    assembler.add_blocks([_block(0, [0, 1, 2, 3, 4]), _block(1, [5, 6, 7, 8, 9])])
    assembler.add_blocks([_block(0, [99, 0, 1, 2, 3])])

    assert [row.item_id for row in assembler.rows()] == [
        "fork_99",
        "fork_0",
        "fork_1",
        "fork_2",
        "fork_3",
        "fork_4",
        "fork_5",
        "fork_6",
        "fork_7",
        "fork_8",
        "fork_9",
    ]


def test_protocol_assembler_warns_when_partial_recapture_is_ambiguous():
    assembler = ProtocolAssembler()

    assembler.add_blocks([_block(0, [0, 1, 2, 3, 4]), _block(1, [5, 6, 7, 8, 9])])
    assembler.add_blocks([_block(0, [90, 91, 92, 93, 94])])

    assert [row.item_id for row in assembler.rows()] == [f"fork_{index}" for index in range(10)]
    assert assembler.warnings[0].code == "ambiguous_snapshot_merge"
