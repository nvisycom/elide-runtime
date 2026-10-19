"""Unit tests for config, schema translation/projection, and security."""

import bentoml
import pytest
from bento_core.ner.v1 import (
    ClassificationSpec,
    EntitySpec,
    FieldSpec,
    Schema,
    StructureSpec,
)
from bento_gliner2 import config


@pytest.fixture(autouse=True)
def _clear_env(monkeypatch):
    for var in (
        "ELIDE_BENTO_NER_MODEL",
        "ELIDE_BENTO_NER_QUANTIZE",
        "ELIDE_BENTO_NER_COMPILE",
        "ELIDE_BENTO_NER_MAX_TOKENS",
    ):
        monkeypatch.delenv(var, raising=False)


def test_config_defaults():
    assert config.model_id() == config.DEFAULT_MODEL
    assert config.max_tokens() == config.DEFAULT_MAX_TOKENS
    assert config.quantize() is False
    assert config.compile_model() is False


def test_config_reads_env(monkeypatch):
    monkeypatch.setenv("ELIDE_BENTO_NER_MODEL", "org/custom")
    monkeypatch.setenv("ELIDE_BENTO_NER_QUANTIZE", "true")
    monkeypatch.setenv("ELIDE_BENTO_NER_MAX_TOKENS", "256")
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

    from bento_gliner2.engine import build_schema

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
    from bento_gliner2.engine import project

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
    from bento_gliner2.engine import project

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

    from bento_gliner2 import engine

    src = inspect.getsource(engine)
    assert "GLiNER2API" not in src
    assert "api_client" not in src
    assert "from_api" not in src


def test_service_exposes_recognize_endpoint():
    # Importing the service must not require gliner2 (model loads lazily in
    # __init__, not at import).
    from bento_gliner2.service import NerService

    assert isinstance(NerService, bentoml.Service)
    assert NerService.name == "bento-gliner2"
    assert "recognize" in NerService.apis


def test_project_carries_token_usage():
    """`project` attaches the usage it is handed, and omits it otherwise."""
    from bento_core.ner.v1 import TokenUsage
    from bento_gliner2.engine import project

    schema = Schema(entities=[EntitySpec(label="person")])
    result = {"entities": {}}

    assert project(result, schema, "fastino/x").tokens is None

    resp = project(result, schema, "fastino/x", tokens=TokenUsage(input=12, limit=512))
    assert resp.tokens.input == 12
    assert resp.tokens.limit == 512


def test_check_length_returns_the_count_it_measured():
    """The limit check returns its count so usage costs no second encode.

    A fake tokenizer counts its calls: one `check_length` must encode exactly
    once, since the reported usage reuses that same measurement.
    """
    from bento_gliner2.engine import Engine, TextTooLongError

    class _FakeTokenizer:
        def __init__(self) -> None:
            self.calls = 0

        def encode(self, text: str) -> list[int]:
            self.calls += 1
            return list(range(len(text.split())))

    engine = Engine.__new__(Engine)  # no model load
    engine.max_tokens = 5
    engine._tokenizer = _FakeTokenizer()

    assert engine.check_length("one two three") == 3
    assert engine._tokenizer.calls == 1

    with pytest.raises(TextTooLongError):
        engine.check_length("a b c d e f g")
