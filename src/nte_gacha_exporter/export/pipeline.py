from __future__ import annotations

import json
from pathlib import Path
from typing import cast

from nte_gacha_exporter.core.models import ParsedRow, ParseWarning
from nte_gacha_exporter.core.schema import ExportDocument, RawPacketRecord
from nte_gacha_exporter.decode.binary import parse_packet_record
from nte_gacha_exporter.export.assembler import ProtocolAssembler
from nte_gacha_exporter.export.document import ExportOptions, build_document
from nte_gacha_exporter.mapping.runtime import load_locale_map


def read_raw_capture(path: Path) -> tuple[list[ParsedRow], list[ParseWarning]]:
    """Read raw packet JSONL and return decoded rows plus warnings."""

    assembler = ProtocolAssembler()
    warnings: list[ParseWarning] = []
    session_index = -1
    packet_index = 0
    in_session = False
    saw_session = False

    with path.open("r", encoding="utf-8", errors="replace") as fh:
        for line_no, line in enumerate(fh, 1):
            if not line.strip():
                continue
            try:
                record = json.loads(line)
            except json.JSONDecodeError as exc:
                warnings.append(ParseWarning("bad_jsonl", f"line {line_no}: {exc}", line=line_no))
                continue
            if not isinstance(record, dict):
                warnings.append(ParseWarning("bad_jsonl", f"line {line_no}: record is not an object", line=line_no))
                continue

            typ = record.get("type")
            if typ == "capture_start":
                saw_session = True
                in_session = True
                session_index += 1
                packet_index = 0
            elif typ == "capture_stop":
                in_session = False
            elif typ == "packet" and in_session:
                blocks, found_warnings = parse_packet_record(
                    cast(RawPacketRecord, record),
                    session=session_index,
                    line=line_no,
                    packet_index=packet_index,
                )
                packet_index += 1
                warnings.extend(found_warnings)
                for block in blocks:
                    assembler.add_block(block)

    if not saw_session:
        raise ValueError("raw capture has no capture_start records")

    rows = assembler.rows()
    warnings.extend(assembler.warnings)
    return rows, warnings


def export_capture(
    raw_path: Path,
    *,
    locale: str,
) -> ExportDocument:
    rows, warnings = read_raw_capture(raw_path)
    locale_name, mapping = load_locale_map(locale)
    return build_document(
        rows,
        mapping,
        ExportOptions(locale=locale_name, source=str(raw_path)),
        warnings,
    )
