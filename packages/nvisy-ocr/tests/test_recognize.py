"""Integration test for the OCR recognize endpoint, with docTR faked.

Drives the real BentoML service via ``to_asgi()`` + Starlette's TestClient so we
exercise request validation, the batchable list contract, and the docTR-output
-> Page/Block/Line/Word mapping (normalized geometry -> pixel boxes, confidence
filtering) — without downloading torch or any weights.
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


class _FakeDoctr:
    """Returns one page with one block/line and two words in docTR's shape."""

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
    os.environ["NVISY_MODEL_NAME"] = "fakedet+fakerec"

    doctr_models = types.ModuleType("doctr.models")
    doctr_models.ocr_predictor = lambda *a, **k: _FakeDoctr()
    doctr_pkg = types.ModuleType("doctr")
    doctr_pkg.models = doctr_models
    sys.modules["doctr"] = doctr_pkg
    sys.modules["doctr.models"] = doctr_models

    from nvisy_ocr.service import OcrService
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
        json={"requests": [{"image": _png_b64(), "confidenceThreshold": 0.5}]},
    )
    assert resp.status_code == 200
    body = resp.json()[0]
    assert body["modelId"] == "fakedet+fakerec"
    # camelCase regression guard.
    page = body["pages"][0]
    assert "pageNumber" in page and "page_number" not in page
    words = page["blocks"][0]["lines"][0]["words"]
    # "Lovelace" (0.40) dropped by the 0.5 threshold; "Ada" stays.
    assert [w["text"] for w in words] == ["Ada"]
    # normalized (0,0)-(0.3,0.1) on a 100x50 page -> pixel box.
    assert words[0]["bbox"] == {"x": 0.0, "y": 0.0, "width": 30.0, "height": 5.0}


def test_block_kind_defaults_to_text(client):
    resp = client.post(
        "/recognize",
        json={"requests": [{"image": _png_b64(), "confidenceThreshold": 0.0}]},
    )
    assert resp.json()[0]["pages"][0]["blocks"][0]["kind"] == "text"


def test_rejects_invalid_base64(client):
    resp = client.post(
        "/recognize",
        json={"requests": [{"image": "not!base64", "confidenceThreshold": 0.0}]},
    )
    assert resp.status_code == 400
