"""Integration test for the VL recognize endpoint, with PaddleOCR-VL faked.

Drives the real BentoML service via ``to_asgi()`` + Starlette's TestClient so we
exercise request validation, the batchable list contract, and the parsing_res
-> Region mapping (bbox, block_label -> BlockKind, reading order) — without
loading PaddleOCR-VL or any weights.
"""

from __future__ import annotations

import base64
import io
import os
import sys
import types
from types import SimpleNamespace

import pytest


def _block(bbox, label, content):
    # Mirrors PaddleOCRVLBlock: an object with bbox/label/content attributes.
    return SimpleNamespace(bbox=bbox, label=label, content=content)


class _FakeVL:
    def __init__(self, *args, **kwargs):
        pass

    def predict(self, _image):
        # PaddleOCRVLResult is dict-like; parsing_res_list holds block objects in
        # reading order (the list index is the reading-order index).
        return [
            {
                "parsing_res_list": [
                    _block([0, 0, 80, 15], "title", "first"),  # title -> TEXT
                    _block([0, 20, 60, 40], "text", "second"),
                    _block([0, 50, 90, 70], "mystery", "third"),  # unknown -> OTHER
                ]
            }
        ]


@pytest.fixture(scope="module")
def client():
    _reset_prometheus_registry()
    os.environ["NVISY_MODEL_NAME"] = "fake/vl"
    os.environ["NVISY_DEVICE"] = "cpu"

    paddleocr = types.ModuleType("paddleocr")
    paddleocr.PaddleOCRVL = _FakeVL
    sys.modules["paddleocr"] = paddleocr

    from nvisy_vl.service import OcrVlService
    from starlette.testclient import TestClient

    with TestClient(OcrVlService.to_asgi()) as c:
        yield c


def _reset_prometheus_registry() -> None:
    from prometheus_client import REGISTRY

    for collector in list(REGISTRY._collector_to_names):
        REGISTRY.unregister(collector)


def _png_b64() -> str:
    from PIL import Image

    buf = io.BytesIO()
    Image.new("RGB", (100, 80), "white").save(buf, format="PNG")
    return base64.b64encode(buf.getvalue()).decode()


def test_recognize_maps_regions_sorted_by_reading_order(client):
    resp = client.post("/recognize", json={"requests": [{"image": _png_b64()}]})
    assert resp.status_code == 200
    body = resp.json()[0]
    assert body["modelId"] == "fake/vl"
    regions = body["regions"]
    # reading order = list index.
    assert [r["text"] for r in regions] == ["first", "second", "third"]
    assert [r["readingOrder"] for r in regions] == [0, 1, 2]
    # title -> text, mystery -> other.
    assert regions[0]["kind"] == "text"
    assert regions[2]["kind"] == "other"
    # bbox from [x1,y1,x2,y2].
    assert regions[0]["bbox"] == {"x": 0.0, "y": 0.0, "width": 80.0, "height": 15.0}


def test_rejects_invalid_base64(client):
    resp = client.post("/recognize", json={"requests": [{"image": "not!base64"}]})
    assert resp.status_code == 400
