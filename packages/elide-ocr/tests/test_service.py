"""Smoke tests for the OnnxTR OCR service."""

import bentoml
from elide_ocr.service import OcrService


def test_service_exposes_recognize_endpoint():
    assert isinstance(OcrService, bentoml.Service)
    assert OcrService.name == "elide-ocr"
    assert "recognize" in OcrService.apis
