"""Smoke tests for the docTR OCR service."""

import bentoml
from elide_bento_ocr.service import OcrService


def test_service_exposes_recognize_endpoint():
    assert isinstance(OcrService, bentoml.Service)
    assert OcrService.name == "elide-bento-ocr"
    assert "recognize" in OcrService.apis
