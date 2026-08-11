"""Smoke tests for the docTR OCR service."""

import bentoml
from nvisy_ocr.service import OcrService


def test_service_exposes_recognize_endpoint():
    assert isinstance(OcrService, bentoml.Service)
    assert OcrService.name == "nvisy-inference-ocr"
    assert "recognize" in OcrService.apis
