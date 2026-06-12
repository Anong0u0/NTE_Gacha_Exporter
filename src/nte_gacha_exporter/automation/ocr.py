from __future__ import annotations

import asyncio
import contextlib
import re
import tempfile
from dataclasses import dataclass
from pathlib import Path

from nte_gacha_exporter.automation.errors import AutomationEnvironmentError

PAGE_RE = re.compile(r"(?P<current>\d+)\D+(?P<total>\d+)")
OCR_TRANSLATION = str.maketrans(
    {
        "O": "0",
        "o": "0",
        "I": "1",
        "l": "1",
        "|": "1",
        "／": "/",
        "：": ":",
        "０": "0",
        "１": "1",
        "２": "2",
        "３": "3",
        "４": "4",
        "５": "5",
        "６": "6",
        "７": "7",
        "８": "8",
        "９": "9",
    }
)


@dataclass(frozen=True)
class PageNumber:
    current: int
    total: int
    text: str


def parse_page_text(text: str) -> PageNumber:
    normalized = text.translate(OCR_TRANSLATION)
    match = PAGE_RE.search(normalized)
    if match is None:
        digits = re.findall(r"\d+", normalized)
        if len(digits) >= 2:
            current, total = int(digits[0]), int(digits[1])
        else:
            raise AutomationEnvironmentError(f"cannot parse page text: {text!r}")
    else:
        current = int(match.group("current"))
        total = int(match.group("total"))
    if current <= 0 or total <= 0 or current > total:
        raise AutomationEnvironmentError(f"invalid page number: {current}/{total}")
    return PageNumber(current=current, total=total, text=text)


class WindowsOcrClient:
    def __init__(self, *, language: str = "en-US") -> None:
        self.language = language

    def read_text(self, image: object) -> str:
        return asyncio.run(self._read_text(image))

    def read_page_number(self, image: object) -> PageNumber:
        errors: list[str] = []
        for candidate in _page_number_candidates(image):
            try:
                return parse_page_text(self.read_text(candidate))
            except AutomationEnvironmentError as exc:
                errors.append(str(exc))
        detail = "; ".join(errors) if errors else "no OCR candidates"
        raise AutomationEnvironmentError(f"cannot read page number: {detail}")

    async def _read_text(self, image: object) -> str:
        try:
            import winrt.windows.storage.streams  # noqa: F401
            from winrt.windows.globalization import Language
            from winrt.windows.graphics.imaging import BitmapDecoder
            from winrt.windows.media.ocr import OcrEngine
            from winrt.windows.storage import FileAccessMode, StorageFile
        except Exception as exc:
            raise AutomationEnvironmentError(f"Windows OCR unavailable: {exc}") from exc

        temp_path = _write_temp_png(image)
        try:
            storage_file = await StorageFile.get_file_from_path_async(str(temp_path))
            stream = await storage_file.open_async(FileAccessMode.READ)
            decoder = await BitmapDecoder.create_async(stream)
            bitmap = await decoder.get_software_bitmap_async()
            engine = OcrEngine.try_create_from_language(Language(self.language))
            if engine is None:
                engine = OcrEngine.try_create_from_user_profile_languages()
            if engine is None:
                raise AutomationEnvironmentError("Windows OCR engine is unavailable")
            result = await engine.recognize_async(bitmap)
            return "\n".join(str(line.text) for line in result.lines)
        finally:
            with contextlib.suppress(OSError):
                temp_path.unlink()


def _write_temp_png(image: object) -> Path:
    image = _prepare_ocr_image(image)
    try:
        save = image.save  # type: ignore[attr-defined]
    except AttributeError as exc:
        raise AutomationEnvironmentError("OCR input must be a PIL-compatible image") from exc
    with tempfile.NamedTemporaryFile(prefix="nte_ocr_", suffix=".png", delete=False) as temp:
        temp_path = Path(temp.name)
    save(temp_path)
    return temp_path


def _prepare_ocr_image(image: object) -> object:
    try:
        from PIL import Image
    except Exception:
        return image
    if not isinstance(image, Image.Image):
        return image
    return image.convert("RGB")


def _page_number_candidates(image: object) -> list[object]:
    candidates = [_prepare_ocr_image(image)]
    try:
        from PIL import Image, ImageOps
    except Exception:
        return candidates
    if not isinstance(image, Image.Image):
        return candidates

    rgb = image.convert("RGB")
    scale = 4
    scaled_size = (rgb.width * scale, rgb.height * scale)
    candidates.append(rgb.resize(scaled_size, Image.Resampling.LANCZOS))
    gray = image.convert("L")
    high_contrast = ImageOps.autocontrast(gray).resize(scaled_size, Image.Resampling.LANCZOS).convert("RGB")
    candidates.append(high_contrast)
    inverted = ImageOps.invert(ImageOps.autocontrast(gray)).resize(scaled_size, Image.Resampling.LANCZOS).convert("RGB")
    candidates.append(inverted)
    return candidates
