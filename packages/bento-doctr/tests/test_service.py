"""Smoke tests for the docTR OCR service."""

import bentoml
from bento_doctr.service import OcrService


def test_service_exposes_recognize_endpoint():
    assert isinstance(OcrService, bentoml.Service)
    assert OcrService.name == "bento-doctr"
    assert "recognize" in OcrService.apis
