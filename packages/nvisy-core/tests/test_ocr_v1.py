"""Tests for the OCR v1 wire contract."""

import pytest
from nvisy_core.ocr.v1 import (
    Block,
    BlockKind,
    BoundingBox,
    Line,
    OcrRequest,
    OcrResponse,
    Page,
    Word,
)


def _bbox() -> BoundingBox:
    return BoundingBox(x=0, y=0, width=10, height=4)


def test_request_defaults_threshold_zero():
    req = OcrRequest(image="")
    assert req.confidence_threshold == 0.0


def test_block_defaults_to_text_kind():
    block = Block(text="hi", bbox=_bbox())
    assert block.kind == BlockKind.TEXT


def test_response_serializes_camel_case_with_model_id():
    word = Word(text="Ada", confidence=0.9, bbox=_bbox())
    line = Line(text="Ada", bbox=_bbox(), words=[word])
    block = Block(text="Ada", kind=BlockKind.TEXT, bbox=_bbox(), lines=[line])
    page = Page(page_number=1, width=100.0, height=50.0, blocks=[block])
    resp = OcrResponse(pages=[page], model_id="doctr/db_resnet50+crnn")

    dumped = resp.model_dump(by_alias=True, mode="json")
    assert dumped["modelId"] == "doctr/db_resnet50+crnn"
    assert dumped["pages"][0]["pageNumber"] == 1
    assert dumped["pages"][0]["blocks"][0]["kind"] == "text"


def test_polygon_requires_four_points():
    quad = ((0, 0), (10, 0), (10, 4), (0, 4))
    word = Word(text="x", bbox=_bbox(), polygon=quad)
    assert len(word.polygon) == 4
    with pytest.raises(ValueError):
        Word(text="x", bbox=_bbox(), polygon=((0, 0), (1, 1)))  # only 2 points


def test_confidence_bounded():
    with pytest.raises(ValueError):
        Word(text="x", confidence=1.5, bbox=_bbox())


def test_bbox_rejects_negative_size():
    with pytest.raises(ValueError):
        BoundingBox(x=0, y=0, width=-1, height=4)
