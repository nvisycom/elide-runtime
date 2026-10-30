"""Decode images and perform OCR independently of HTTP handling."""

from elide_core.ocr.v1 import OcrRequest, OcrResponse

from elide_ocr.inference.backends.base import OcrBackend
from elide_ocr.inference.validation import _decode_image


class OcrPipeline:
    def __init__(self, backend: OcrBackend) -> None:
        self.backend = backend

    def recognize(self, requests: list[OcrRequest], rid: str | None = None) -> list[OcrResponse]:
        # One predictor call per image, not one per window: on CPU a batched
        # call measured about twice as slow, with a higher memory peak. See
        # docs/design/ocr.md, "Request and execution flow".
        return [
            self.backend.recognize(_decode_image(req.image, rid), req.confidence_threshold)
            for req in requests
        ]
