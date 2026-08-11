"""Smoke + block-map tests for the VL service."""

import bentoml
from elide_bento_core.ocr.v1 import BlockKind
from elide_bento_vl.block_map import block_kind
from elide_bento_vl.service import OcrVlService


def test_service_exposes_recognize_endpoint():
    assert isinstance(OcrVlService, bentoml.Service)
    assert OcrVlService.name == "elide-bento-vl"
    assert "recognize" in OcrVlService.apis


def test_block_kind_maps_known_labels():
    assert block_kind("text") == BlockKind.TEXT
    assert block_kind("title") == BlockKind.TEXT
    assert block_kind("table") == BlockKind.TABLE
    assert block_kind("figure") == BlockKind.FIGURE
    assert block_kind("chart") == BlockKind.FIGURE


def test_block_kind_unknown_falls_back_to_other():
    assert block_kind("formula") == BlockKind.OTHER
    assert block_kind("totally-unknown-label") == BlockKind.OTHER


def test_block_kind_normalizes_case_and_space():
    assert block_kind("  Table  ") == BlockKind.TABLE
