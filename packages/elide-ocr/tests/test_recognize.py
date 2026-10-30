"""Integration test for the OCR recognize endpoint, with OnnxTR faked.

Drives the real BentoML service via ``to_asgi()`` + Starlette's TestClient so we
exercise request validation, the batchable list contract, and the OnnxTR-output
-> Page/Block/Line/Word mapping (normalized geometry -> pixel boxes, confidence
filtering) — without downloading any weights.
"""

from __future__ import annotations

import base64
import io
import os
import sys
import types
from types import SimpleNamespace

import pytest


def _word(value, conf, geom):
    return SimpleNamespace(value=value, confidence=conf, geometry=geom)


class _FakeOnnxtr:
    """Returns one page with one block/line and two words in OnnxTR's shape."""

    def __call__(self, images):
        # geometry is normalized ((xmin,ymin),(xmax,ymax)); dimensions=(h, w).
        ada = _word("Ada", 0.99, ((0.0, 0.0), (0.3, 0.1)))
        lovelace = _word("Lovelace", 0.40, ((0.0, 0.12), (0.6, 0.22)))
        line = SimpleNamespace(geometry=((0.0, 0.0), (0.6, 0.22)), words=[ada, lovelace])
        block = SimpleNamespace(geometry=((0.0, 0.0), (0.6, 0.22)), lines=[line])
        page = SimpleNamespace(dimensions=(50, 100), blocks=[block])
        return SimpleNamespace(pages=[page])


@pytest.fixture(scope="module")
def client():
    _reset_prometheus_registry()
    os.environ["ELIDE_BENTO_OCR_DETECTION"] = "fakedet"
    os.environ["ELIDE_BENTO_OCR_RECOGNITION"] = "fakerec"

    onnxtr_models = types.ModuleType("onnxtr.models")
    onnxtr_models.EngineConfig = lambda *a, **k: None
    onnxtr_models.ocr_predictor = lambda *a, **k: _FakeOnnxtr()
    onnxtr_pkg = types.ModuleType("onnxtr")
    onnxtr_pkg.models = onnxtr_models
    sys.modules["onnxtr"] = onnxtr_pkg
    sys.modules["onnxtr.models"] = onnxtr_models

    from elide_ocr.service import OcrService
    from starlette.testclient import TestClient

    with TestClient(OcrService.to_asgi()) as c:
        yield c


def _reset_prometheus_registry() -> None:
    from prometheus_client import REGISTRY

    for collector in list(REGISTRY._collector_to_names):
        REGISTRY.unregister(collector)


def _png_b64() -> str:
    from PIL import Image

    buf = io.BytesIO()
    Image.new("RGB", (100, 50), "white").save(buf, format="PNG")
    return base64.b64encode(buf.getvalue()).decode()


def test_recognize_maps_hierarchy_and_filters_confidence(client):
    resp = client.post(
        "/recognize",
        json={"requests": [{"image": _png_b64(), "confidence_threshold": 0.5}]},
    )
    assert resp.status_code == 200
    body = resp.json()[0]
    assert body["model_id"] == "fakedet+fakerec"
    # Snake_case wire names, matching the generated client.
    page = body["pages"][0]
    assert "page_number" in page and "pageNumber" not in page
    words = page["blocks"][0]["lines"][0]["words"]
    # "Lovelace" (0.40) dropped by the 0.5 threshold; "Ada" stays.
    assert [w["text"] for w in words] == ["Ada"]
    # normalized (0,0)-(0.3,0.1) on a 100x50 page -> pixel box.
    assert words[0]["bbox"] == {"x": 0.0, "y": 0.0, "width": 30.0, "height": 5.0}


def test_block_kind_defaults_to_text(client):
    resp = client.post(
        "/recognize",
        json={"requests": [{"image": _png_b64(), "confidence_threshold": 0.0}]},
    )
    assert resp.json()[0]["pages"][0]["blocks"][0]["kind"] == "text"


def test_rejects_invalid_base64(client):
    resp = client.post(
        "/recognize",
        json={"requests": [{"image": "not!base64", "confidence_threshold": 0.0}]},
    )
    assert resp.status_code == 400


@pytest.mark.parametrize("kind", ["truncated", "corrupt"])
def test_rejects_invalid_image_data_with_400(client, kind):
    from PIL import Image

    if kind == "truncated":
        buffer = io.BytesIO()
        Image.new("RGB", (50, 50), "white").save(buffer, format="BMP")
        data = buffer.getvalue()[:-50]
    else:
        data = bytearray(base64.b64decode(_png_b64()))
        data[data.index(b"IDAT") + 4] = 0
    response = client.post(
        "/recognize", json={"requests": [{"image": base64.b64encode(data).decode()}]}
    )
    assert response.status_code == 400


def test_rejects_decompression_bomb_with_400(client, monkeypatch):
    from PIL import Image

    payload = _png_b64()
    monkeypatch.setattr(Image, "MAX_IMAGE_PIXELS", 100)
    response = client.post("/recognize", json={"requests": [{"image": payload}]})
    assert response.status_code == 400


def test_operational_image_failure_remains_500(client, monkeypatch):
    import errno

    from PIL import Image

    payload = _png_b64()

    def fail(*args, **kwargs):
        raise OSError(errno.ENOMEM, "out of memory")

    monkeypatch.setattr(Image.Image, "convert", fail)
    response = client.post("/recognize", json={"requests": [{"image": payload}]})
    assert response.status_code == 500
