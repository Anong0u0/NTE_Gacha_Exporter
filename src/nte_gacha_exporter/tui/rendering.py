from __future__ import annotations

import json
import time
import uuid
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from rich.align import Align
from rich.console import Console, Group, RenderableType
from rich.panel import Panel
from rich.table import Table
from rich.text import Text

from nte_gacha_exporter.app.operations import OperationResult, result_path_lines
from nte_gacha_exporter.app.summary import (
    add_capture_counts,
    empty_capture_counts,
    format_capture_counts,
    offline_capture_counts,
    record_line,
    summary_text,
)
from nte_gacha_exporter.core.models import GachaRecord
from nte_gacha_exporter.core.schema import ExportDocument, LocalizationMap
from nte_gacha_exporter.tui.i18n import TuiI18n

TUI_FRAME_WIDTH = 76
TUI_DISPLAY_SCHEMA_VERSION = 1


def centeredPanel(
    renderable: RenderableType,
    *,
    title: str,
    width: int = TUI_FRAME_WIDTH,
    subtitle: str | None = None,
) -> Align:
    panel = Panel(renderable, title=title, subtitle=subtitle, expand=False, width=width)
    return Align.center(panel)


def writeJsonAtomic(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temp_path = path.with_name(f"{path.name}.{uuid.uuid4().hex}.tmp")
    temp_path.write_text(json.dumps(payload, ensure_ascii=False), encoding="utf-8")
    temp_path.replace(path)


def readJsonObject(path: Path) -> dict[str, Any] | None:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (FileNotFoundError, json.JSONDecodeError, OSError):
        return None
    return data if isinstance(data, dict) else None


def recordPayload(record: GachaRecord | dict[str, Any]) -> dict[str, Any]:
    if isinstance(record, GachaRecord):
        return record.to_dict()
    return dict(record)


def recentRecords(
    records: list[GachaRecord] | list[dict[str, Any]] | tuple[dict[str, Any], ...],
) -> tuple[dict[str, Any], ...]:
    return tuple(recordPayload(record) for record in records[-10:])


def documentRecords(document: ExportDocument | None) -> tuple[dict[str, Any], ...]:
    if not document:
        return ()
    records = document.get("nte", {}).get("list", [])
    return recentRecords(records)


def operationResultPayload(result: OperationResult) -> dict[str, Any]:
    return {
        "exitCode": result.exitCode,
        "lines": list(result.lines),
        "document": result.document,
        "paths": {key: str(path) for key, path in result.paths.items()},
        "error": result.error,
        "captureCounts": result.captureCounts,
        "lastRecords": list(result.lastRecords),
    }


def operationResultFromPayload(payload: dict[str, Any]) -> OperationResult:
    paths = {str(key): Path(str(value)) for key, value in dict(payload.get("paths") or {}).items() if value is not None}
    document = payload.get("document")
    if not isinstance(document, dict) and paths.get("json"):
        document = readJsonObject(paths["json"])
    raw_records = payload.get("lastRecords")
    last_records = (
        tuple(record for record in raw_records if isinstance(record, dict)) if isinstance(raw_records, list) else ()
    )
    if not last_records and isinstance(document, dict):
        last_records = documentRecords(document)
    return OperationResult(
        int(payload.get("exitCode") or 0),
        lines=tuple(str(line) for line in payload.get("lines") or ()),
        document=document if isinstance(document, dict) else None,
        paths=paths,
        error=str(payload["error"]) if payload.get("error") else None,
        captureCounts=str(payload["captureCounts"]) if payload.get("captureCounts") else None,
        lastRecords=last_records,
    )


@dataclass
class TuiDisplayState:
    state: str = "idle"
    titleKey: str = "title"
    status: str = ""
    captureCounts: str = ""
    paths: dict[str, Path] = field(default_factory=dict)
    handoffContext: dict[str, Any] = field(default_factory=dict)
    lastRecords: tuple[dict[str, Any], ...] = ()
    result: OperationResult | None = None
    error: str | None = None
    updatedAt: float = field(default_factory=time.time)

    @classmethod
    def fromResult(
        cls,
        result: OperationResult,
        *,
        titleKey: str = "done",
        mapping: LocalizationMap | None = None,
    ) -> TuiDisplayState:
        capture_counts = result.captureCounts
        if not capture_counts and result.document and mapping is not None:
            capture_counts = offline_capture_counts(result.document, mapping)
        records = result.lastRecords or documentRecords(result.document)
        return cls(
            state="error" if result.exitCode != 0 or result.error else "completed",
            titleKey=titleKey,
            status=result.error or "",
            captureCounts=capture_counts or "",
            paths=dict(result.paths),
            lastRecords=records,
            result=result
            if result.lastRecords == records and result.captureCounts == capture_counts
            else resultWithDisplayData(result, capture_counts, records),
            error=result.error,
        )

    @classmethod
    def fromPayload(cls, payload: dict[str, Any]) -> TuiDisplayState:
        result_payload = payload.get("result")
        result = operationResultFromPayload(result_payload) if isinstance(result_payload, dict) else None
        paths = {
            str(key): Path(str(value)) for key, value in dict(payload.get("paths") or {}).items() if value is not None
        }
        records_payload = payload.get("lastRecords")
        records = (
            tuple(record for record in records_payload if isinstance(record, dict))
            if isinstance(records_payload, list)
            else ()
        )
        if result and not records:
            records = result.lastRecords or documentRecords(result.document)
        capture_counts = str(payload.get("captureCounts") or "")
        if result and not capture_counts:
            capture_counts = result.captureCounts or ""
        if result:
            result = resultWithDisplayData(
                result, capture_counts or result.captureCounts, records or result.lastRecords
            )
        return cls(
            state=str(payload.get("state") or "idle"),
            titleKey=str(payload.get("titleKey") or "title"),
            status=str(payload.get("status") or ""),
            captureCounts=capture_counts,
            paths=paths or (dict(result.paths) if result else {}),
            handoffContext=dict(payload.get("handoffContext") or {}),
            lastRecords=records,
            result=result,
            error=str(payload["error"]) if payload.get("error") else (result.error if result else None),
            updatedAt=float(payload.get("updatedAt") or time.time()),
        )

    def replaceRecords(self, records: list[GachaRecord] | list[dict[str, Any]], mapping: LocalizationMap) -> None:
        counts = empty_capture_counts()
        add_capture_counts(counts, records)
        self.captureCounts = format_capture_counts(mapping, counts)
        self.lastRecords = recentRecords(records)
        self.updatedAt = time.time()

    def replaceDocument(self, document: ExportDocument, mapping: LocalizationMap) -> None:
        self.captureCounts = offline_capture_counts(document, mapping)
        self.lastRecords = documentRecords(document)
        self.updatedAt = time.time()

    def attachResult(self, result: OperationResult) -> None:
        self.result = resultWithDisplayData(
            result, self.captureCounts or result.captureCounts, self.lastRecords or result.lastRecords
        )
        self.error = self.result.error
        self.updatedAt = time.time()

    def toPayload(self) -> dict[str, Any]:
        return {
            "kind": "tuiDisplayState",
            "schemaVersion": TUI_DISPLAY_SCHEMA_VERSION,
            "state": self.state,
            "titleKey": self.titleKey,
            "status": self.status,
            "captureCounts": self.captureCounts,
            "paths": {key: str(path) for key, path in self.paths.items()},
            "handoffContext": self.handoffContext,
            "lastRecords": list(self.lastRecords),
            "error": self.error,
            "updatedAt": self.updatedAt,
            "result": operationResultPayload(self.result) if self.result else None,
        }


def resultWithDisplayData(
    result: OperationResult,
    captureCounts: str | None,
    lastRecords: tuple[dict[str, Any], ...],
) -> OperationResult:
    return OperationResult(
        result.exitCode,
        lines=result.lines,
        document=result.document,
        paths=result.paths,
        error=result.error,
        captureCounts=captureCounts,
        lastRecords=lastRecords,
    )


class TuiDisplayStateWriter:
    def __init__(self, path: Path) -> None:
        self.path = path

    def write(self, state: TuiDisplayState) -> None:
        state.updatedAt = time.time()
        writeJsonAtomic(self.path, state.toPayload())


def readDisplayState(path: Path) -> TuiDisplayState | None:
    payload = readJsonObject(path)
    return TuiDisplayState.fromPayload(payload) if payload else None


class TuiRenderer:
    def __init__(self, console: Console, i18n: TuiI18n, *, frameWidth: int = TUI_FRAME_WIDTH) -> None:
        self.console = console
        self.i18n = i18n
        self.frameWidth = frameWidth

    def frame_width(self) -> int:
        return max(20, min(self.frameWidth, self.console.width))

    def content_divider(self) -> str:
        return "_" * max(16, min(58, self.frame_width() - 14))

    def prompt(self, prompt: str) -> str:
        prefix = " " * max(0, (self.console.width - self.frame_width()) // 2)
        return f"{prefix}{prompt}"

    def printFrame(self, renderable: RenderableType, *, titleKey: str, subtitleKey: str | None = None) -> None:
        self.console.print(
            centeredPanel(
                renderable,
                title=self.i18n.text(titleKey),
                subtitle=self.i18n.text(subtitleKey) if subtitleKey else None,
                width=self.frame_width(),
            )
        )

    def printFrameText(self, renderable: RenderableType, *, title: str, subtitle: str | None = None) -> None:
        self.console.print(centeredPanel(renderable, title=title, subtitle=subtitle, width=self.frame_width()))

    def printCentered(self, renderable: RenderableType) -> None:
        self.console.print(Align.center(renderable))

    def statusText(self, value: bool) -> Text:
        label = self.i18n.text("enabled") if value else self.i18n.text("disabled")
        return Text(label, style="bold green" if value else "bold red")

    def valueLine(self, labelKey: str, value: object) -> Text:
        return Text.assemble((f"{self.i18n.text(labelKey)}: ", "bold"), str(value))

    def toggleLine(self, labelKey: str, value: bool) -> Text:
        return Text.assemble((f"{self.i18n.text(labelKey)}: ", "bold"), self.statusText(value))

    def renderMenu(
        self,
        *,
        headingKey: str | None = None,
        items: list[tuple[str, str]],
        footerItems: list[tuple[str, str]] | None = None,
        settings: list[RenderableType] | None = None,
    ) -> Group:
        lines: list[RenderableType] = []
        if settings:
            lines.extend(settings)
            lines.extend(["", self.content_divider(), ""])
        if headingKey:
            lines.extend([f"{self.i18n.text(headingKey)}:", ""])
        lines.extend(f"  [{key}] {self.i18n.text(labelKey)}" for key, labelKey in items)
        if footerItems:
            lines.extend(["", self.content_divider(), ""])
            lines.extend(f"  [{key}] {self.i18n.text(labelKey)}" for key, labelKey in footerItems)
        return Group(*lines)

    def renderCaptureState(self, state: TuiDisplayState) -> Align:
        lines: list[RenderableType] = [
            Text(state.status or self.i18n.text("running")),
            Text(f"state={state.state}"),
        ]
        if state.captureCounts:
            lines.append(Text(state.captureCounts))
        lines.extend(Text(line) for line in result_path_lines(state.paths))
        table = self.recordsTable(state.lastRecords)
        if table:
            lines.append(table)
        return centeredPanel(Group(*lines), title=self.i18n.text(state.titleKey), width=self.frame_width())

    def printResult(self, result: OperationResult) -> None:
        self.printResultState(TuiDisplayState.fromResult(result))

    def printResultState(self, state: TuiDisplayState) -> None:
        result = state.result
        title_key = "done"
        if state.error or (result and (result.exitCode != 0 or result.error)):
            title_key = "error"
        lines: list[str] = []
        if result:
            lines.extend(result.lines)
            if result.error:
                lines.append(result.error)
            lines.extend(result_path_lines(result.paths))
            if result.document:
                lines.insert(
                    0, summary_text(result.document, capture_counts=state.captureCounts or result.captureCounts)
                )
        else:
            if state.error:
                lines.append(state.error)
            if state.status:
                lines.append(state.status)
            lines.extend(result_path_lines(state.paths))
        self.printFrame("\n".join(lines) if lines else self.i18n.text(title_key), titleKey=title_key)
        table = self.recordsTable(state.lastRecords, title=self.i18n.text("lastRecords"))
        if table:
            self.printCentered(table)

    def recordsTable(self, records: tuple[dict[str, Any], ...], *, title: str | None = None) -> Table | None:
        if not records:
            return None
        table = Table(title=title, expand=False, width=max(20, self.frame_width() - 4))
        table.add_column("time")
        table.add_column("record")
        for record in records[-10:]:
            text = record_line(record)
            time_text, _, rest = text.partition(" | ")
            table.add_row(time_text, rest)
        return table
