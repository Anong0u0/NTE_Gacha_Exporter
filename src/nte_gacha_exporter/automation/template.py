from __future__ import annotations

from dataclasses import dataclass

from nte_gacha_exporter.automation.errors import AutomationEnvironmentError
from nte_gacha_exporter.automation.profile import AutomationProfile, Point, Rect, Size, template_resource_path


@dataclass(frozen=True)
class TemplateMatch:
    name: str
    matched: bool
    score: float
    point: Point


class ImageTemplateMatcher:
    def __init__(self, profile: AutomationProfile) -> None:
        self.profile = profile

    def verify(self, name: str, screen_image: object) -> TemplateMatch:
        return self._match(name, screen_image, prefer_expected=True)

    def find(self, name: str, screen_image: object) -> TemplateMatch:
        return self._match(name, screen_image, prefer_expected=False)

    def _match(self, name: str, screen_image: object, *, prefer_expected: bool) -> TemplateMatch:
        spec = self.profile.templates[name]
        template = _load_template_image(template_resource_path(spec))
        template = _resize_template_to_rect(template, spec.rect)
        match = find_template(
            name=name,
            screen_image=screen_image,
            template_image=template,
            expected_rect=spec.rect,
            search_padding=spec.searchPadding,
            threshold=spec.threshold,
            prefer_expected=prefer_expected,
        )
        if not match.matched:
            raise AutomationEnvironmentError(f"screen template not found: {name} score={match.score:.2f}")
        return match

    def verify_exact_crop(self, name: str, crop_image: object) -> TemplateMatch:
        spec = self.profile.templates[name]
        template = _load_template_image(template_resource_path(spec))
        template = _resize_template_to_rect(template, spec.rect)
        crop = _to_rgb(crop_image)
        template_size = _image_size(template)
        crop_size = _image_size(crop)
        if crop_size != template_size:
            raise AutomationEnvironmentError(
                f"template crop size mismatch: {name} expected={template_size.width}x{template_size.height} "
                f"got={crop_size.width}x{crop_size.height}"
            )
        score = _mean_abs_diff(crop, template)
        match = TemplateMatch(name, score <= spec.threshold, score, Point(spec.rect.x, spec.rect.y))
        if not match.matched:
            raise AutomationEnvironmentError(f"screen template not found: {name} score={match.score:.2f}")
        return match


def find_template(
    *,
    name: str,
    screen_image: object,
    template_image: object,
    expected_rect: Rect,
    search_padding: Point,
    threshold: float,
    step: int = 2,
    prefer_expected: bool = False,
) -> TemplateMatch:
    screen = _to_rgb(screen_image)
    template = _to_rgb(template_image)
    screen_size = _image_size(screen)
    template_size = _image_size(template)
    search_rect = expected_rect.expand(search_padding).clamp(screen_size)
    if template_size.width > search_rect.width or template_size.height > search_rect.height:
        return TemplateMatch(name, False, float("inf"), Point(search_rect.x, search_rect.y))
    if prefer_expected:
        expected = expected_rect.clamp(screen_size)
        if expected.width >= template_size.width and expected.height >= template_size.height:
            score = _mean_abs_diff(_crop(screen, expected.x, expected.y, template_size), template)
            return TemplateMatch(name, score <= threshold, score, Point(expected.x, expected.y))
        return TemplateMatch(name, False, float("inf"), Point(expected.x, expected.y))

    best_score = float("inf")
    best_point = Point(search_rect.x, search_rect.y)
    max_x = search_rect.right - template_size.width
    max_y = search_rect.bottom - template_size.height
    for y in _positions(search_rect.y, max_y, step):
        for x in _positions(search_rect.x, max_x, step):
            score = _mean_abs_diff(_crop(screen, x, y, template_size), template)
            if score < best_score:
                best_score = score
                best_point = Point(x, y)
                if best_score == 0:
                    return TemplateMatch(name, True, best_score, best_point)
    return TemplateMatch(name, best_score <= threshold, best_score, best_point)


def _load_template_image(path):
    try:
        from PIL import Image
    except Exception as exc:
        raise AutomationEnvironmentError(f"Pillow unavailable for template matching: {exc}") from exc
    try:
        return Image.open(path).convert("RGB")
    except Exception as exc:
        raise AutomationEnvironmentError(f"cannot load template {path}: {exc}") from exc


def _resize_template_to_rect(image: object, rect: Rect) -> object:
    size = _image_size(image)
    if size.width == rect.width and size.height == rect.height:
        return image
    try:
        from PIL import Image

        resample = Image.Resampling.BILINEAR
    except Exception:
        resample = 2
    try:
        return image.resize((rect.width, rect.height), resample)  # type: ignore[attr-defined]
    except AttributeError as exc:
        raise AutomationEnvironmentError("template image must support resize") from exc


def _to_rgb(image: object) -> object:
    try:
        return image.convert("RGB")  # type: ignore[attr-defined]
    except AttributeError as exc:
        raise AutomationEnvironmentError("template input must be a PIL-compatible image") from exc


def _image_size(image: object) -> Size:
    size = image.size  # type: ignore[attr-defined]
    return Size(int(size[0]), int(size[1]))


def _crop(image: object, x: int, y: int, size: Size) -> object:
    return image.crop((x, y, x + size.width, y + size.height))  # type: ignore[attr-defined]


def _mean_abs_diff(left: object, right: object) -> float:
    left_bytes = left.tobytes()  # type: ignore[attr-defined]
    right_bytes = right.tobytes()  # type: ignore[attr-defined]
    if len(left_bytes) != len(right_bytes):
        return float("inf")
    return sum(abs(a - b) for a, b in zip(left_bytes, right_bytes, strict=True)) / max(1, len(left_bytes))


def _positions(start: int, stop: int, step: int) -> list[int]:
    if stop <= start:
        return [start]
    positions = list(range(start, stop + 1, max(1, step)))
    if positions[-1] != stop:
        positions.append(stop)
    return positions
