"""Unit tests for config, schema translation/projection, and security."""

import bentoml
import pytest
from nvisy_core.ner.v1 import (
    ClassificationSpec,
    EntitySpec,
    FieldSpec,
    Schema,
    StructureSpec,
)
from nvisy_ner import config


@pytest.fixture(autouse=True)
def _clear_env(monkeypatch):
    for var in (
        "NVISY_NER_MODEL",
        "NVISY_NER_QUANTIZE",
        "NVISY_NER_COMPILE",
        "NVISY_NER_MAX_TOKENS",
    ):
        monkeypatch.delenv(var, raising=False)


def test_config_defaults():
    assert config.model_id() == config.DEFAULT_MODEL
    assert config.max_tokens() == config.DEFAULT_MAX_TOKENS
    assert config.quantize() is False
    assert config.compile_model() is False


def test_config_reads_env(monkeypatch):
    monkeypatch.setenv("NVISY_NER_MODEL", "org/custom")
    monkeypatch.setenv("NVISY_NER_QUANTIZE", "true")
    monkeypatch.setenv("NVISY_NER_MAX_TOKENS", "256")
    assert config.model_id() == "org/custom"
    assert config.quantize() is True
    assert config.max_tokens() == 256


def test_build_schema_translates_all_groups():
    # build_schema only needs gliner2.Schema/RegexValidator; fake them.
    import sys
    import types

    class _S:
        def __init__(self):
            self.calls = []

        def entities(self, m):
            self.calls.append(("entities", m))
            return self

        def classification(self, t, labels, multi_label=False, cls_threshold=None):
            self.calls.append(("cls", t, tuple(labels), multi_label, cls_threshold))
            return self

        def structure(self, name):
            self.calls.append(("struct", name))
            return _B(self, name)

    class _B:
        def __init__(self, parent, name):
            self.parent = parent

        def field(self, name, **kw):
            self.parent.calls.append(("field", name, kw.get("dtype"), bool(kw.get("validators"))))
            return self

    mod = types.ModuleType("gliner2")
    mod.Schema = _S
    mod.RegexValidator = lambda *a, **k: object()
    sys.modules["gliner2"] = mod

    from nvisy_ner.engine import build_schema

    schema = Schema(
        entities=[
            EntitySpec(label="person", description="a name"),
            EntitySpec(label="email", threshold=0.9),  # threshold -> dict form
        ],
        classifications=[
            ClassificationSpec(task="lang", labels=["en", "fr"], multi_label=True, threshold=0.7),
        ],
        structures=[StructureSpec(name="c", fields=[FieldSpec(name="email", pattern="x")])],
    )
    g = build_schema(schema)
    # bare label keeps its description string; a thresholded label uses the dict form
    assert ("entities", {"person": "a name", "email": {"threshold": 0.9}}) in g.calls
    # per-task threshold flows through as cls_threshold
    assert ("cls", "lang", ("en", "fr"), True, 0.7) in g.calls
    assert ("struct", "c") in g.calls
    # the field carries a regex validator (pattern set)
    assert ("field", "email", "list", True) in g.calls


def test_project_maps_confidence_to_score():
    from nvisy_ner.engine import project

    schema = Schema(
        entities=[EntitySpec(label="person")],
        classifications=[
            ClassificationSpec(task="sentiment", labels=["pos"]),
            ClassificationSpec(task="topics", labels=["a"], multi_label=True),
        ],
        structures=[StructureSpec(name="contact", fields=[FieldSpec(name="name")])],
    )
    result = {
        "entities": {"person": [{"text": "Ada", "confidence": 0.9, "start": 0, "end": 3}]},
        "sentiment": {"label": "pos", "confidence": 0.7},
        "topics": [{"label": "a", "confidence": 0.6}],
        "contact": [{"name": [{"text": "Ada", "confidence": 0.8, "start": 0, "end": 3}]}],
    }
    resp = project(result, schema, "fastino/x")
    assert resp.entities[0].score == 0.9 and resp.entities[0].label == "person"
    assert resp.classifications["sentiment"].label == "pos"
    assert resp.classifications["topics"][0].score == 0.6
    assert resp.structures["contact"][0]["name"][0].text == "Ada"
    assert resp.model_id == "fastino/x"


def test_project_routes_empty_groups_by_schema():
    # An empty classification and an empty structure both come back as []; route
    # them by the schema's declared types, not by sniffing the value shape.
    from nvisy_ner.engine import project

    schema = Schema(
        classifications=[ClassificationSpec(task="topic", labels=["a", "b"])],
        structures=[StructureSpec(name="rec", fields=[FieldSpec(name="f")])],
    )
    result = {"topic": [], "rec": []}
    resp = project(result, schema, "fastino/x")
    # "rec" is a structure (empty list of records), not a classification.
    assert resp.structures == {"rec": []}
    assert resp.classifications == {"topic": []}


def test_engine_does_not_use_the_hosted_api():
    # Security: the local engine path must never reference gliner2's hosted API
    # client. Guard against a regression that would route data off-box.
    import inspect

    from nvisy_ner import engine

    src = inspect.getsource(engine)
    assert "GLiNER2API" not in src
    assert "api_client" not in src
    assert "from_api" not in src


def test_service_exposes_recognize_endpoint():
    # Importing the service must not require gliner2 (model loads lazily in
    # __init__, not at import).
    from nvisy_ner.service import NerService

    assert isinstance(NerService, bentoml.Service)
    assert NerService.name == "nvisy-inference-ner"
    assert "recognize" in NerService.apis
