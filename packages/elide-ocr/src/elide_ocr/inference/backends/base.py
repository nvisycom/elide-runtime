"""Contract implemented by OCR model backends."""

from __future__ import annotations

from typing import TYPE_CHECKING, Protocol

from elide_core.ocr.v1 import OcrResponse

if TYPE_CHECKING:
    from PIL.Image import Image


class OcrBackend(Protocol):
    model_id: str

    def recognize(self, image: Image, threshold: float) -> OcrResponse: ...
