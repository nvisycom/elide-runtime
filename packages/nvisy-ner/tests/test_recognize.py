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
        self.classifications = []
        self.structures = []

    def entities(self, mapping):
        self.entities_arg = mapping
        return self

    def classification(self, task, labels, multi_label=False, cls_threshold=None):
        self.classifications.append((task, tuple(labels), multi_label))
        return self

    def structure(self, name):
        builder = _FakeStructureBuilder(name)
        self.structures.append(builder)
        return builder


class _FakeStructureBuilder:
    def __init__(self, name):
        self.name = name
        self.fields = []

    def field(self, name, **kwargs):
        self.fields.append((name, kwargs))
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
            if schema.entities_arg and "person" in schema.entities_arg:
                result["entities"] = {
                    "person": [{"text": "Ada", "confidence": 0.95, "start": 0, "end": 3}]
                }
            for task, labels, multi in schema.classifications:
                if multi:
                    result[task] = [{"label": labels[0], "confidence": 0.8}]
                else:
                    result[task] = {"label": labels[0], "confidence": 0.8}
            for sb in schema.structures:
                result[sb.name] = [
                    {
                        f: [{"text": "Ada", "confidence": 0.9, "start": 0, "end": 3}]
                        for f, _ in sb.fields
                    }
                ]
            out.append(result)
        return out


@pytest.fixture(scope="module")
def client():
    _reset_prometheus_registry()
    os.environ["NVISY_NER_MODEL"] = _MODEL_ID
    os.environ["NVISY_NER_MAX_TOKENS"] = "5"  # tiny, to exercise the guard

    gliner2 = types.ModuleType("gliner2")
    gliner2.GLiNER2 = _FakeGLiNER2
    gliner2.Schema = _FakeSchema
    gliner2.RegexValidator = lambda *a, **k: ("regex", a, k)
    sys.modules["gliner2"] = gliner2

    from nvisy_ner.service import NerService
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
    assert body["modelId"] == _MODEL_ID
    assert body["entities"][0] == {
        "text": "Ada",
        "label": "person",
        "score": 0.95,
        "start": 0,
        "end": 3,
    }


def test_classification_single_and_multi(client):
    single = _post(
        client, text="Ada", schema={"classifications": [{"task": "s", "labels": ["pos"]}]}
    )
    assert single.json()[0]["classifications"]["s"] == {"label": "pos", "score": 0.8}
    multi = _post(
        client,
        text="Ada",
        schema={"classifications": [{"task": "t", "labels": ["a"], "multiLabel": True}]},
    )
    assert multi.json()[0]["classifications"]["t"] == [{"label": "a", "score": 0.8}]


def test_structured_record(client):
    resp = _post(
        client,
        text="Ada",
        schema={"structures": [{"name": "contact", "fields": [{"name": "name"}]}]},
    )
    assert resp.status_code == 200
    rec = resp.json()[0]["structures"]["contact"][0]
    assert rec["name"][0]["text"] == "Ada"


def test_over_length_rejected(client):
    # max tokens is 5; 6 words → 400.
    resp = _post(client, text="a b c d e f", schema={"entities": [{"label": "person"}]})
    assert resp.status_code == 400


def test_empty_schema_rejected(client):
    resp = _post(client, text="Ada", schema={})
    assert resp.status_code == 400


def test_mixed_schemas_dispatched(client):
    # Two different schemas in one batch each get their own result.
    resp = client.post(
        "/recognize",
        json={
            "requests": [
                {"text": "Ada", "schema": {"entities": [{"label": "person"}]}},
                {"text": "Ada", "schema": {"classifications": [{"task": "s", "labels": ["pos"]}]}},
            ]
        },
    )
    assert resp.status_code == 200
    bodies = resp.json()
    assert bodies[0]["entities"] and not bodies[0]["classifications"]
    assert bodies[1]["classifications"] and not bodies[1]["entities"]
