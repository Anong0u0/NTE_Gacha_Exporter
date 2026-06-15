from __future__ import annotations

import sys
import time
from collections.abc import Callable
from dataclasses import dataclass
from threading import Event
from typing import Any

from nte_gacha_exporter.automation import winapi
from nte_gacha_exporter.automation.errors import AutomationEnvironmentError
from nte_gacha_exporter.automation.ocr import PageNumber, WindowsOcrClient
from nte_gacha_exporter.automation.profile import Point, WorkflowStep, load_profile
from nte_gacha_exporter.automation.screen import WindowCaptureClient
from nte_gacha_exporter.automation.template import ImageTemplateMatcher
from nte_gacha_exporter.automation.tooltip import AutomationTooltip
from nte_gacha_exporter.capture.windows_net import CaptureTarget


@dataclass(frozen=True)
class AutoPageStatus:
    elapsedSeconds: float
    message: str
    kind: str = "status"
    step: str | None = None
    pool: str | None = None
    currentPage: int | None = None
    totalPages: int | None = None
    technicalDetail: str = ""
    replaceable: bool = True


StatusCallback = Callable[[AutoPageStatus], None]
StatusFormatter = Callable[[AutoPageStatus], str]
RecordSnapshot = Callable[[], list[dict[str, Any]]]


@dataclass(frozen=True)
class AutoPageOptions:
    target: CaptureTarget
    stop_event: Event
    full_update: bool = False
    known_record_ids: tuple[str, ...] = ()
    record_snapshot: RecordSnapshot | None = None
    non_interactive: bool = False
    on_status: StatusCallback | None = None
    status_formatter: StatusFormatter | None = None
    click_timeout: float = 1.5
    click_poll_interval: float = 0.3
    duplicate_check_timeout: float = 1.5
    template_timeout: float = 5.0
    tooltip: bool = True


@dataclass(frozen=True)
class AutoPageResult:
    status: str
    message: str
    completedPools: tuple[str, ...] = ()
    skippedPools: tuple[str, ...] = ()

    @property
    def succeeded(self) -> bool:
        return self.status == "completed"

    @property
    def manual(self) -> bool:
        return self.status == "manual"


def run_auto_page(options: AutoPageOptions) -> AutoPageResult:
    try:
        pager = AutoPager(options)
        return pager.run()
    except AutomationEnvironmentError as exc:
        if options.non_interactive:
            return AutoPageResult("failed", str(exc))
        return AutoPageResult("manual", str(exc))


class AutoPager:
    def __init__(self, options: AutoPageOptions) -> None:
        self.options = options
        profile = load_profile()
        winapi.require_windows()
        self.window = winapi.resolve_game_window(options.target.pid, class_name=profile.window.className)
        self.profile = profile.scaled(self.window.clientSize)
        self.capture = WindowCaptureClient(self.window.hwnd)
        self.ocr = WindowsOcrClient()
        self.matcher = ImageTemplateMatcher(self.profile)
        self.started_at = time.monotonic()
        self.tooltip = AutomationTooltip(enabled=options.tooltip)
        if self.tooltip.unavailable_reason:
            self._status(
                "tooltip unavailable",
                kind="diagnostic",
                technicalDetail=self.tooltip.unavailable_reason,
                replaceable=False,
            )
        if self.window.clientSize.width != 1920 or self.window.clientSize.height != 1080:
            self._status(
                f"client={self.window.clientSize.width}x{self.window.clientSize.height}; "
                "non-1920x1080 auto page may be inaccurate",
                kind="diagnostic",
                replaceable=False,
            )

    def run(self) -> AutoPageResult:
        completed: list[str] = []
        skipped: list[str] = []
        self._status("auto page started", kind="started", step="started")
        self._focus_window()
        try:
            for step in self.profile.workflow:
                if self._should_stop():
                    return AutoPageResult("manual", "auto page stopped", tuple(completed), tuple(skipped))
                page_result = self._run_step(step)
                if page_result:
                    pool, was_skipped = page_result
                    if was_skipped:
                        skipped.append(pool)
                    else:
                        completed.append(pool)
            self.options.stop_event.set()
            self._status("auto page completed", kind="completed", step="completed")
            return AutoPageResult("completed", "auto page completed", tuple(completed), tuple(skipped))
        finally:
            self.tooltip.close()

    def _run_step(self, step: WorkflowStep) -> tuple[str, bool] | None:
        if step.status:
            self._status(step.status, kind="step", step=step.status)
        if step.action == "verifyTemplate":
            self._verify_template(_required(step.template, "template"), step.status)
            return None
        if step.action == "click":
            self._click(self._point(_required(step.point, "point")), settle=step.settle)
            return None
        if step.action == "clickUntilTemplate":
            self._click_until_template(step)
            return None
        if step.action == "clickTemplateUntilTemplate":
            self._click_template_until_template(step)
            return None
        if step.action == "pressEsc":
            winapi.foreground_escape(self.window)
            time.sleep(0.1)
            return None
        if step.action == "page":
            return self._capture_pages(step)
        raise AutomationEnvironmentError(f"unsupported workflow action: {step.action}")

    def _verify_template(self, name: str, step: str | None) -> None:
        started = time.monotonic()
        match, attempts = self._wait_for_template(name)
        elapsed = time.monotonic() - started
        self._status(
            "template verified",
            kind="template",
            step=step,
            technicalDetail=(
                f"{name} score={match.score:.2f} at={match.point.x},{match.point.y} "
                f"wait={elapsed:.2f}s tries={attempts}"
            ),
        )

    def _wait_for_template(self, name: str):
        deadline = time.monotonic() + self.options.template_timeout
        last_error: Exception | None = None
        attempts = 0
        next_focus_at = 0.0
        while time.monotonic() < deadline:
            if self._should_stop():
                raise AutomationEnvironmentError("auto page stopped")
            now = time.monotonic()
            if now >= next_focus_at:
                self._focus_window()
                next_focus_at = time.monotonic() + 0.5
            attempts += 1
            try:
                return self._try_template(name), attempts
            except AutomationEnvironmentError as exc:
                last_error = exc
                time.sleep(self.options.click_poll_interval)
        detail = f": {last_error}" if last_error else ""
        raise AutomationEnvironmentError(f"screen template not found after wait: {name}{detail}")

    def _click_until_template(self, step: WorkflowStep) -> None:
        template = _required(step.template, "template")
        points = tuple(self._point(name) for name in step.pointSequence)
        if not points:
            raise AutomationEnvironmentError("workflow step missing pointSequence")

        interval = 0.1 if step.settle is None else step.settle
        deadline = time.monotonic() + self.options.template_timeout
        started = time.monotonic()
        clicks = 0
        last_error: Exception | None = None
        while time.monotonic() < deadline:
            if self._should_stop():
                raise AutomationEnvironmentError("auto page stopped")
            for point in points:
                if time.monotonic() >= deadline:
                    break
                self._click(point, settle=interval)
                clicks += 1
                try:
                    match = self._try_template(template)
                except AutomationEnvironmentError as exc:
                    last_error = exc
                    continue
                elapsed = time.monotonic() - started
                self._status(
                    "template verified",
                    kind="template",
                    step=step.status,
                    technicalDetail=(
                        f"{template} score={match.score:.2f} at={match.point.x},{match.point.y} "
                        f"wait={elapsed:.2f}s clicks={clicks}"
                    ),
                )
                return

        detail = f": {last_error}" if last_error else ""
        raise AutomationEnvironmentError(f"screen template not found after click sequence: {template}{detail}")

    def _click_template_until_template(self, step: WorkflowStep) -> None:
        source_template = _required(step.template, "template")
        target_template = _required(step.targetTemplate, "targetTemplate")
        interval = 0.1 if step.settle is None else step.settle
        deadline = time.monotonic() + self.options.template_timeout
        started = time.monotonic()
        clicks = 0
        source = None
        clicked_source = False
        last_source_error: Exception | None = None
        last_target_error: Exception | None = None
        while time.monotonic() < deadline:
            if self._should_stop():
                raise AutomationEnvironmentError("auto page stopped")

            try:
                target = self._try_template(target_template)
            except AutomationEnvironmentError as exc:
                last_target_error = exc
            else:
                elapsed = time.monotonic() - started
                source_detail = (
                    f"{source_template} at={source.point.x},{source.point.y} "
                    if source is not None
                    else f"{source_template} already resolved "
                )
                self._status(
                    "template verified",
                    kind="template",
                    step=step.status,
                    technicalDetail=(
                        f"{source_detail}{target_template} score={target.score:.2f} "
                        f"at={target.point.x},{target.point.y} wait={elapsed:.2f}s clicks={clicks}"
                    ),
                )
                return

            if clicked_source:
                time.sleep(self.options.click_poll_interval)
                continue

            try:
                source = self._find_template(source_template)
            except AutomationEnvironmentError as exc:
                last_source_error = exc
                time.sleep(self.options.click_poll_interval)
                continue
            click_point = (
                self._point(step.point) if step.point else self._template_center(source_template, source.point)
            )
            self._click(click_point, settle=interval)
            clicks += 1
            clicked_source = True

        last_error = last_target_error or last_source_error
        detail = f": {last_error}" if last_error else ""
        raise AutomationEnvironmentError(
            f"screen template not found after template click sequence: {source_template}->{target_template}{detail}"
        )

    def _capture_pages(self, step: WorkflowStep) -> tuple[str, bool]:
        pool = _required(step.pool, "pool")
        page_rect = self.profile.rects[_required(step.pageRect, "pageRect")]
        next_button = self._point(_required(step.nextButton, "nextButton"))
        page = self._read_page(page_rect)
        self._status(
            "page ready",
            kind="page",
            step=step.status,
            pool=pool,
            currentPage=page.current,
            totalPages=page.total,
        )

        if page.current != 1:
            raise AutomationEnvironmentError(
                f"{pool}: freshly opened record page must be 1/{page.total}, got {page.current}/{page.total}"
            )
        if self._should_skip_pool(pool=pool, step=step.status, page=page):
            return pool, True

        while page.current < page.total:
            if self._should_stop():
                raise AutomationEnvironmentError("auto page stopped")
            expected = page.current + 1
            self._status(
                "page next",
                kind="page",
                step=step.status,
                pool=pool,
                currentPage=expected,
                totalPages=page.total,
            )
            page = self._click_page_button(page_rect, next_button, page, expected)
            if self._should_skip_pool(pool=pool, step=step.status, page=page):
                return pool, True

        self._status(
            "pool completed",
            kind="pool_completed",
            step=step.status,
            pool=pool,
            currentPage=page.total,
            totalPages=page.total,
        )
        return pool, False

    def _should_skip_pool(self, *, pool: str, step: str, page: PageNumber) -> bool:
        if self.options.full_update or not self.options.known_record_ids or self.options.record_snapshot is None:
            return False

        known_ids = set(self.options.known_record_ids)
        deadline = time.monotonic() + self.options.duplicate_check_timeout
        while time.monotonic() <= deadline:
            page_records = self._latest_pool_page_records(pool)
            if len(page_records) >= 5:
                record_ids = [str(record.get("record_id") or "") for record in page_records[-5:]]
                if record_ids and all(record_id in known_ids for record_id in record_ids):
                    self._status(
                        "known page found; skipping pool",
                        kind="pool_skipped",
                        step=step,
                        pool=pool,
                        currentPage=page.current,
                        totalPages=page.total,
                    )
                    return True
                return False
            time.sleep(self.options.click_poll_interval)
        return False

    def _latest_pool_page_records(self, pool: str) -> list[dict[str, Any]]:
        if self.options.record_snapshot is None:
            return []
        records = self.options.record_snapshot()
        pool_records = [record for record in records if _record_pool(record) == pool]
        return pool_records[-5:]

    def _click_page_button(
        self,
        page_rect,
        point: Point,
        previous: PageNumber,
        expected_page: int,
    ) -> PageNumber:
        max_attempts = 2
        for attempt in range(1, max_attempts + 1):
            winapi.foreground_click(self.window, point, down_time=0.06)
            page = self._wait_for_page(page_rect, previous.current, expected_page)
            if page is not None:
                return page
            if attempt < max_attempts:
                self._status(
                    "page did not change; retrying click",
                    kind="retry",
                    currentPage=previous.current,
                    totalPages=previous.total,
                    technicalDetail=f"attempt={attempt + 1}/{max_attempts}",
                )
        raise AutomationEnvironmentError(
            f"page did not change after retry: expected {expected_page}, still {previous.current}"
        )

    def _wait_for_page(
        self,
        page_rect,
        previous_page: int,
        expected_page: int,
    ) -> PageNumber | None:
        deadline = time.monotonic() + self.options.click_timeout
        last_error: Exception | None = None
        saw_previous = False
        while time.monotonic() < deadline:
            if self._should_stop():
                raise AutomationEnvironmentError("auto page stopped")
            time.sleep(self.options.click_poll_interval)
            try:
                page = self._read_page(page_rect)
            except AutomationEnvironmentError as exc:
                last_error = exc
                continue
            if page.current == expected_page:
                return page
            if page.current == previous_page:
                saw_previous = True
            else:
                raise AutomationEnvironmentError(f"unexpected page after click: {page.current}/{page.total}")
        if last_error is not None:
            self._status("OCR waiting ended", kind="diagnostic", technicalDetail=str(last_error))
        if last_error is not None and not saw_previous:
            raise AutomationEnvironmentError(f"OCR unreadable after click: {last_error}")
        return None

    def _click(self, point: Point, *, settle: float | None = None) -> None:
        winapi.foreground_click(self.window, point)
        time.sleep(0.1 if settle is None else settle)

    def _try_template(self, name: str):
        image = self.capture.capture_rect(self.profile.templates[name].rect)
        return self.matcher.verify_exact_crop(name, image)

    def _find_template(self, name: str):
        self._focus_window()
        image = self.capture.capture_client(self.window.clientSize)
        return self.matcher.find(name, image)

    def _template_center(self, name: str, top_left: Point) -> Point:
        rect = self.profile.templates[name].rect
        return Point(top_left.x + rect.width // 2, top_left.y + rect.height // 2)

    def _read_page(self, page_rect) -> PageNumber:
        self._focus_window()
        image = self.capture.capture_rect(page_rect)
        return self.ocr.read_page_number(image)

    def _point(self, name: str) -> Point:
        return self.profile.points[name]

    def _should_stop(self) -> bool:
        return self.options.stop_event.is_set() or winapi.escape_pressed()

    def _focus_window(self) -> None:
        self.window = winapi.refresh_window(self.window)
        winapi.force_foreground(self.window)

    def _status(
        self,
        message: str,
        *,
        kind: str = "status",
        step: str | None = None,
        pool: str | None = None,
        currentPage: int | None = None,
        totalPages: int | None = None,
        technicalDetail: str = "",
        replaceable: bool = True,
    ) -> None:
        status = AutoPageStatus(
            elapsedSeconds=time.monotonic() - self.started_at,
            message=message,
            kind=kind,
            step=step,
            pool=pool,
            currentPage=currentPage,
            totalPages=totalPages,
            technicalDetail=technicalDetail,
            replaceable=replaceable,
        )
        if self.options.status_formatter:
            display_message = self.options.status_formatter(status)
        else:
            display_message = _status_text(status)
        self.tooltip.show(display_message)
        if self.options.on_status:
            self.options.on_status(status)
        elif sys.stderr:
            print(f"+{status.elapsedSeconds:.2f}s {display_message}", file=sys.stderr)


def _required(value: str | None, name: str) -> str:
    if value is None:
        raise AutomationEnvironmentError(f"workflow step missing {name}")
    return value


def _status_text(status: AutoPageStatus) -> str:
    text = status.message
    if status.currentPage is not None and status.totalPages is not None:
        text = f"{text} page={status.currentPage}/{status.totalPages}"
    if status.technicalDetail:
        return f"{text}: {status.technicalDetail}"
    return text


def _record_pool(record: dict[str, Any]) -> str | None:
    pool_id = str(record.get("pool_id") or "")
    record_type = str(record.get("record_type") or "")
    if pool_id == "CardPool_Character":
        return "limited"
    if pool_id == "CardPool_NewRole":
        return "standard"
    if record_type == "fork" or pool_id.startswith("ForkLottery_"):
        return "fork"
    return None
