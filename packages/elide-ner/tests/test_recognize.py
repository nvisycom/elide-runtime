"""Integration test for the NER recognize endpoint, with GLiNER2 faked.

Drives the real BentoML service via ``to_asgi()`` + Starlette's TestClient so we
exercise request validation, the schema -> gliner2.Schema translation, the
result projection, over-length rejection, and schema-grouped batching — without
downloading torch, gliner2, or any weights.
"""

from __future__ import annotations

import os
import sys
import types

import pytest

_MODEL_ID = "fastino/gliner2-privacy-filter-PII-multi"


class _FakeSchema:
    """Records the schema calls so assertions can check the translation."""

    def __init__(self):
        self.entities_arg = None

    def entities(self, mapping):
        self.entities_arg = mapping
        return self


class _FakeTokenizer:
    def encode(self, text):
        # 1 "token" per whitespace-split word — enough to exercise the limit.
        return text.split()


class _FakeGLiNER2:
    """Mirrors the verified gliner2 result shape for batch_extract."""

    processor = types.SimpleNamespace(tokenizer=_FakeTokenizer())

    @classmethod
    def from_pretrained(cls, *args, **kwargs):
        return cls()

    def batch_extract(self, texts, schema, threshold=0.5, max_len=None, **kwargs):
        out = []
        for _ in texts:
            result = {}
            # One span per requested label, so a test can tell which schema
            # produced a response — a hardcoded label cannot distinguish them.
            if schema.entities_arg:
                result["entities"] = {
                    label: [{"text": "Ada", "confidence": 0.95, "start": 0, "end": 3}]
                    for label in schema.entities_arg
                }
            out.append(result)
        return out


@pytest.fixture(scope="module")
def client():
    _reset_prometheus_registry()
    os.environ["ELIDE_BENTO_NER_MODEL"] = _MODEL_ID
    os.environ["ELIDE_BENTO_NER_MAX_TOKENS"] = "5"  # tiny, to exercise the guard

    gliner2 = types.ModuleType("gliner2")
    gliner2.GLiNER2 = _FakeGLiNER2
    gliner2.Schema = _FakeSchema
    gliner2.RegexValidator = lambda *a, **k: ("regex", a, k)
    sys.modules["gliner2"] = gliner2

    from elide_ner.service import NerService
    from starlette.testclient import TestClient

    with TestClient(NerService.to_asgi()) as c:
        yield c


def _reset_prometheus_registry() -> None:
    from prometheus_client import REGISTRY

    for collector in list(REGISTRY._collector_to_names):
        REGISTRY.unregister(collector)


def _post(client, **req):
    return client.post("/recognize", json={"requests": [req]})


def test_entities_extracted(client):
    resp = _post(client, text="Ada", schema={"entities": [{"label": "person"}]})
    assert resp.status_code == 200
    body = resp.json()[0]
    assert body["model_id"] == _MODEL_ID
    assert body["entities"][0] == {
        "text": "Ada",
        "label": "person",
        "score": 0.95,
        "start": 0,
        "end": 3,
    }


def test_over_length_rejected(client):
    # max tokens is 5; 6 words → 400.
    resp = _post(client, text="a b c d e f", schema={"entities": [{"label": "person"}]})
    assert resp.status_code == 400


def test_empty_schema_rejected(client):
    resp = _post(client, text="Ada", schema={})
    assert resp.status_code == 400


def test_oversized_schema_is_rejected_before_batching(client, monkeypatch):
    from concurrent.futures import ThreadPoolExecutor

    from elide_ner.inference.backends.gliner2 import Engine

    checked_sizes = []
    original = Engine.check_schema

    def check_schema(self, size):
        checked_sizes.append(size)
        return original(self, size)

    monkeypatch.setattr(Engine, "check_schema", check_schema)
    with ThreadPoolExecutor(max_workers=2) as pool:
        invalid = pool.submit(
            _post,
            client,
            text="Ada",
            schema={"entities": [{"label": "person", "description": "x" * 200_000}]},
        )
        valid = pool.submit(_post, client, text="Ada", schema={"entities": [{"label": "person"}]})
        assert invalid.result().status_code == 400
        response = valid.result()

    assert response.status_code == 200
    assert response.json()[0]["entities"][0]["label"] == "person"
    # Only the valid request reaches the handler's backend guard. Rejection
    # inside that handler would also fail unrelated callers in a shared batch.
    assert len(checked_sizes) == 1
    assert checked_sizes[0] < 1024


def test_mixed_schemas_dispatched(client):
    # Distinct schemas run as separate extractions, and each response must come
    # back against its own request: the handler groups by schema and reassembles
    # by index, so a grouping bug shows up as answers swapped between requests.
    resp = client.post(
        "/recognize",
        json={
            "requests": [
                {"text": "Ada", "schema": {"entities": [{"label": "person"}]}},
                {"text": "Ada", "schema": {"entities": [{"label": "location"}]}},
            ]
        },
    )
    assert resp.status_code == 200
    bodies = resp.json()
    assert [e["label"] for e in bodies[0]["entities"]] == ["person"]
    assert [e["label"] for e in bodies[1]["entities"]] == ["location"]
