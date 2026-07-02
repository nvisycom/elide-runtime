"""Tests for the VL v1 wire contract."""

import pytest
from nvisy_core.ocr.v1 import BlockKind, BoundingBox
from nvisy_core.ocrvl.v1 import Region, VlRequest, VlResponse


def _bbox() -> BoundingBox:
    return BoundingBox(x=0, y=0, width=10, height=4)


def test_region_defaults_text_kind():
    r = Region(text="hi", bbox=_bbox(), reading_order=0)
    assert r.kind == BlockKind.TEXT


def test_response_serializes_camel_case_with_model_id():
    region = Region(text="Ada", kind=BlockKind.TABLE, bbox=_bbox(), reading_order=2, confidence=0.9)
    resp = VlResponse(regions=[region], model_id="PaddlePaddle/PaddleOCR-VL")
    dumped = resp.model_dump(by_alias=True, mode="json")
    assert dumped["modelId"] == "PaddlePaddle/PaddleOCR-VL"
    assert dumped["regions"][0]["readingOrder"] == 2
    assert dumped["regions"][0]["kind"] == "table"


def test_request_round_trips():
    req = VlRequest(image="aGVsbG8=")
    assert req.image == "aGVsbG8="


def test_reading_order_non_negative():
    with pytest.raises(ValueError):
        Region(text="x", bbox=_bbox(), reading_order=-1)
