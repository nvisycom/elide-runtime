"""Real-model tests: exercise the engine against actual GLiNER2 weights.

Marked ``real`` and excluded from the default suite (which fakes the model — no
weight downloads in CI). Run via ``pytest -m real`` (the opt-in ``real-models``
CI job). These catch what fakes cannot: a wrong assumption about the real
gliner2 API would keep the faked suite green while the engine is broken.
"""

from __future__ import annotations

import pytest
from nvisy_core.ner.v1 import (
    ClassificationSpec,
    EntitySpec,
    FieldSpec,
    Schema,
    StructureSpec,
)

pytestmark = pytest.mark.real


@pytest.fixture(scope="module")
def engine():
    from nvisy_ner.engine import Engine

    return Engine()


def test_entities_classification_and_structure(engine):
    schema = Schema(
        entities=[EntitySpec(label="person"), EntitySpec(label="email")],
        classifications=[ClassificationSpec(task="language", labels=["en", "fr", "de"])],
        structures=[
            StructureSpec(name="contact", fields=[FieldSpec(name="name"), FieldSpec(name="email")])
        ],
    )
    [resp] = engine.recognize(
        ["Ada Lovelace, ada@example.com, writes in English."], schema, threshold=0.3
    )

    labels = {e.label: e for e in resp.entities}
    assert "person" in labels
    assert labels["person"].text == "Ada Lovelace"
    assert (labels["person"].start, labels["person"].end) == (0, 12)

    assert resp.classifications["language"].label == "en"

    record = resp.structures["contact"][0]
    assert record["name"][0].text == "Ada Lovelace"


def test_offline_load_and_infer(monkeypatch):
    # The service runs with the Hub offline; loading + inference must work
    # against the baked/cached weights with no network.
    monkeypatch.setenv("HF_HUB_OFFLINE", "1")
    monkeypatch.setenv("TRANSFORMERS_OFFLINE", "1")
    from nvisy_ner.engine import Engine

    engine = Engine()
    [resp] = engine.recognize(["Ada in London"], Schema(entities=[EntitySpec(label="person")]), 0.3)
    assert any(e.label == "person" for e in resp.entities)


def test_over_length_rejected(engine):
    from nvisy_ner.engine import TextTooLongError

    with pytest.raises(TextTooLongError):
        engine.check_length("word " * (engine.max_tokens + 50))
