from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any, cast

from nte_gacha_exporter.automation.errors import AutomationEnvironmentError
from nte_gacha_exporter.resources.json_data import load_json, resource_path

AUTOMATION_PACKAGE = "nte_gacha_exporter.resources.automation"
PROFILE_SCHEMA = "nte-gacha-auto-profile"
PROFILE_SCHEMA_VERSION = 2
DEFAULT_PROFILE = "default"
SUPPORTED_ASPECT_RATIO = "16:9"
ASPECT_RATIO_TOLERANCE = 0.01

POINT_IDS = {
    "homeBoardFile",
    "homeForkEntry",
    "boardRecordTab",
    "boardTypeDropdown",
    "boardLimitedOption",
    "boardStandardOption",
    "boardNextButton",
    "forkDetailFile",
    "forkActivityCard",
    "forkRecordTab",
    "forkNextButton",
}
RECT_IDS = {"boardPageNumber", "forkPageNumber"}
ACTION_IDS = {"verifyTemplate", "click", "clickUntilTemplate", "clickTemplateUntilTemplate", "pressEsc", "page"}


@dataclass(frozen=True)
class Point:
    x: int
    y: int


@dataclass(frozen=True)
class Rect:
    x: int
    y: int
    width: int
    height: int

    @property
    def right(self) -> int:
        return self.x + self.width

    @property
    def bottom(self) -> int:
        return self.y + self.height

    def expand(self, padding: Point) -> Rect:
        return Rect(
            x=self.x - padding.x,
            y=self.y - padding.y,
            width=self.width + padding.x * 2,
            height=self.height + padding.y * 2,
        )

    def clamp(self, size: Size) -> Rect:
        left = min(max(0, self.x), size.width)
        top = min(max(0, self.y), size.height)
        right = min(max(left, self.right), size.width)
        bottom = min(max(top, self.bottom), size.height)
        return Rect(left, top, max(1, right - left), max(1, bottom - top))


@dataclass(frozen=True)
class Size:
    width: int
    height: int


@dataclass(frozen=True)
class WindowSpec:
    exe: str
    className: str


@dataclass(frozen=True)
class TemplateSpec:
    file: str
    rect: Rect
    searchPadding: Point
    threshold: float


@dataclass(frozen=True)
class WorkflowStep:
    action: str
    status: str
    point: str | None = None
    pointSequence: tuple[str, ...] = ()
    template: str | None = None
    targetTemplate: str | None = None
    pageRect: str | None = None
    nextButton: str | None = None
    pool: str | None = None
    settle: float | None = None


@dataclass(frozen=True)
class AutomationProfile:
    schema: str
    schemaVersion: int
    profile: str
    baseClientSize: Size
    aspectRatio: str
    window: WindowSpec
    points: dict[str, Point]
    rects: dict[str, Rect]
    templates: dict[str, TemplateSpec]
    workflow: tuple[WorkflowStep, ...]

    def scaled(self, client_size: Size) -> AutomationProfile:
        ensure_supported_client_size(client_size)
        scale_x = client_size.width / self.baseClientSize.width
        scale_y = client_size.height / self.baseClientSize.height
        return AutomationProfile(
            schema=self.schema,
            schemaVersion=self.schemaVersion,
            profile=self.profile,
            baseClientSize=client_size,
            aspectRatio=self.aspectRatio,
            window=self.window,
            points={key: _scale_point(value, scale_x, scale_y) for key, value in self.points.items()},
            rects={key: _scale_rect(value, scale_x, scale_y) for key, value in self.rects.items()},
            templates={
                key: TemplateSpec(
                    file=value.file,
                    rect=_scale_rect(value.rect, scale_x, scale_y),
                    searchPadding=_scale_point(value.searchPadding, scale_x, scale_y),
                    threshold=value.threshold,
                )
                for key, value in self.templates.items()
            },
            workflow=self.workflow,
        )


def load_profile() -> AutomationProfile:
    profile = _parse_profile(load_json(AUTOMATION_PACKAGE, DEFAULT_PROFILE), source=DEFAULT_PROFILE)
    _validate_profile(profile)
    return profile


def _validate_profile(profile: AutomationProfile) -> None:
    errors: list[str] = []
    if profile.schema != PROFILE_SCHEMA:
        errors.append(f"schema must be {PROFILE_SCHEMA}")
    if profile.schemaVersion != PROFILE_SCHEMA_VERSION:
        errors.append(f"schemaVersion must be {PROFILE_SCHEMA_VERSION}")
    if profile.aspectRatio != SUPPORTED_ASPECT_RATIO:
        errors.append(f"aspectRatio must be {SUPPORTED_ASPECT_RATIO}")
    if profile.baseClientSize.width <= 0 or profile.baseClientSize.height <= 0:
        errors.append("baseClientSize must be positive")
    elif not is_supported_client_size(profile.baseClientSize):
        errors.append("baseClientSize must be 16:9")

    missing_points = sorted(POINT_IDS - set(profile.points))
    if missing_points:
        errors.append(f"missing points: {', '.join(missing_points)}")
    missing_rects = sorted(RECT_IDS - set(profile.rects))
    if missing_rects:
        errors.append(f"missing rects: {', '.join(missing_rects)}")

    for key, point in profile.points.items():
        _validate_point(point, profile.baseClientSize, f"points.{key}", errors)
    for key, rect in profile.rects.items():
        _validate_rect(rect, profile.baseClientSize, f"rects.{key}", errors)
    for key, template in profile.templates.items():
        _validate_rect(template.rect, profile.baseClientSize, f"templates.{key}.rect", errors)
        _validate_point(template.searchPadding, profile.baseClientSize, f"templates.{key}.searchPadding", errors)
        if template.threshold <= 0:
            errors.append(f"templates.{key}.threshold must be positive")
        try:
            template_path = resource_path(AUTOMATION_PACKAGE, template.file)
        except ValueError as exc:
            errors.append(str(exc))
        else:
            if not template_path.is_file():
                errors.append(f"template not found: {template.file}")

    for index, step in enumerate(profile.workflow):
        _validate_workflow_step(index, step, profile, errors)

    if errors:
        raise AutomationEnvironmentError("; ".join(errors))


def ensure_supported_client_size(size: Size) -> None:
    if not is_supported_client_size(size):
        raise AutomationEnvironmentError(f"auto page supports 16:9 game client only, got {size.width}x{size.height}")


def is_supported_client_size(size: Size) -> bool:
    if size.width <= 0 or size.height <= 0:
        return False
    return abs((size.width / size.height) - (16 / 9)) <= ASPECT_RATIO_TOLERANCE


def template_resource_path(template: TemplateSpec) -> Path:
    return resource_path(AUTOMATION_PACKAGE, template.file)


def _validate_workflow_step(
    index: int,
    step: WorkflowStep,
    profile: AutomationProfile,
    errors: list[str],
) -> None:
    prefix = f"workflow[{index}]"
    if step.action not in ACTION_IDS:
        errors.append(f"{prefix}.action must be one of {', '.join(sorted(ACTION_IDS))}")
    if step.action == "verifyTemplate" and step.template not in profile.templates:
        errors.append(f"{prefix}.template references missing template {step.template}")
    if step.action == "click" and step.point not in profile.points:
        errors.append(f"{prefix}.point references missing point {step.point}")
    if step.action == "clickUntilTemplate":
        if step.template not in profile.templates:
            errors.append(f"{prefix}.template references missing template {step.template}")
        if not step.pointSequence:
            errors.append(f"{prefix}.pointSequence is required for clickUntilTemplate action")
        for point in step.pointSequence:
            if point not in profile.points:
                errors.append(f"{prefix}.pointSequence references missing point {point}")
    if step.action == "clickTemplateUntilTemplate":
        if step.template not in profile.templates:
            errors.append(f"{prefix}.template references missing template {step.template}")
        if step.targetTemplate not in profile.templates:
            errors.append(f"{prefix}.targetTemplate references missing template {step.targetTemplate}")
        if step.point is not None and step.point not in profile.points:
            errors.append(f"{prefix}.point references missing point {step.point}")
    if step.action == "page":
        if step.pageRect not in profile.rects:
            errors.append(f"{prefix}.pageRect references missing rect {step.pageRect}")
        if step.nextButton not in profile.points:
            errors.append(f"{prefix}.nextButton references missing point {step.nextButton}")
        if not step.pool:
            errors.append(f"{prefix}.pool is required for page action")
    if step.settle is not None and step.settle < 0:
        errors.append(f"{prefix}.settle must be non-negative")


def _parse_profile(data: object, *, source: str) -> AutomationProfile:
    if not isinstance(data, dict):
        raise AutomationEnvironmentError(f"profile must be an object: {source}")

    try:
        base_size = _parse_size(_object(data.get("baseClientSize"), "baseClientSize"))
        window_data = _object(data.get("window"), "window")
        points_data = _object(data.get("points"), "points")
        rects_data = _object(data.get("rects"), "rects")
        templates_data = _object(data.get("templates"), "templates")
        workflow_data = _list(data.get("workflow"), "workflow")
        return AutomationProfile(
            schema=str(data.get("schema") or ""),
            schemaVersion=int(data.get("schemaVersion") or 0),
            profile=str(data.get("profile") or source),
            baseClientSize=base_size,
            aspectRatio=str(data.get("aspectRatio") or ""),
            window=WindowSpec(
                exe=str(window_data.get("exe") or "HTGame.exe"),
                className=str(window_data.get("className") or "UnrealWindow"),
            ),
            points={key: _parse_point(_object(value, f"points.{key}")) for key, value in points_data.items()},
            rects={key: _parse_rect(_object(value, f"rects.{key}")) for key, value in rects_data.items()},
            templates={key: _parse_template(value, key) for key, value in templates_data.items()},
            workflow=tuple(_parse_workflow_step(step, index) for index, step in enumerate(workflow_data)),
        )
    except (TypeError, ValueError) as exc:
        raise AutomationEnvironmentError(f"invalid automation profile {source}: {exc}") from exc


def _parse_template(data: object, key: str) -> TemplateSpec:
    obj = _object(data, f"templates.{key}")
    return TemplateSpec(
        file=str(obj.get("file") or ""),
        rect=_parse_rect(_object(obj.get("rect"), f"templates.{key}.rect")),
        searchPadding=_parse_point(_object(obj.get("searchPadding"), f"templates.{key}.searchPadding")),
        threshold=float(obj.get("threshold") or 0),
    )


def _parse_workflow_step(data: object, index: int) -> WorkflowStep:
    obj = _object(data, f"workflow[{index}]")
    return WorkflowStep(
        action=str(obj.get("action") or ""),
        status=str(obj.get("status") or ""),
        point=_optional_str(obj.get("point")),
        pointSequence=_optional_str_tuple(obj.get("pointSequence")),
        template=_optional_str(obj.get("template")),
        targetTemplate=_optional_str(obj.get("targetTemplate")),
        pageRect=_optional_str(obj.get("pageRect")),
        nextButton=_optional_str(obj.get("nextButton")),
        pool=_optional_str(obj.get("pool")),
        settle=_optional_float(obj.get("settle")),
    )


def _optional_str(value: object) -> str | None:
    if value is None:
        return None
    text = str(value)
    return text or None


def _optional_str_tuple(value: object) -> tuple[str, ...]:
    if value is None:
        return ()
    return tuple(_optional_str(item) or "" for item in _list(value, "pointSequence"))


def _optional_float(value: object) -> float | None:
    if value is None:
        return None
    return float(value)


def _parse_size(data: dict[str, Any]) -> Size:
    return Size(width=int(data.get("width") or 0), height=int(data.get("height") or 0))


def _parse_point(data: dict[str, Any]) -> Point:
    return Point(x=int(data.get("x") or 0), y=int(data.get("y") or 0))


def _parse_rect(data: dict[str, Any]) -> Rect:
    return Rect(
        x=int(data.get("x") or 0),
        y=int(data.get("y") or 0),
        width=int(data.get("width") or 0),
        height=int(data.get("height") or 0),
    )


def _object(value: object, name: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ValueError(f"{name} must be an object")
    return cast(dict[str, Any], value)


def _list(value: object, name: str) -> list[object]:
    if not isinstance(value, list):
        raise ValueError(f"{name} must be a list")
    return value


def _scale_point(point: Point, scale_x: float, scale_y: float) -> Point:
    return Point(x=round(point.x * scale_x), y=round(point.y * scale_y))


def _scale_rect(rect: Rect, scale_x: float, scale_y: float) -> Rect:
    return Rect(
        x=round(rect.x * scale_x),
        y=round(rect.y * scale_y),
        width=max(1, round(rect.width * scale_x)),
        height=max(1, round(rect.height * scale_y)),
    )


def _validate_point(point: Point, size: Size, name: str, errors: list[str]) -> None:
    if point.x < 0 or point.y < 0 or point.x > size.width or point.y > size.height:
        errors.append(f"{name} is outside baseClientSize")


def _validate_rect(rect: Rect, size: Size, name: str, errors: list[str]) -> None:
    if rect.width <= 0 or rect.height <= 0:
        errors.append(f"{name} size must be positive")
        return
    if rect.x < 0 or rect.y < 0 or rect.right > size.width or rect.bottom > size.height:
        errors.append(f"{name} is outside baseClientSize")
