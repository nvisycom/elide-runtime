"""Real-model tests: exercise the engine against actual GLiNER2 weights.

Marked ``real`` and excluded from the default suite (which fakes the model — no
weight downloads in CI). Run via ``pytest -m real`` (the opt-in ``real-models``
CI job). These catch what fakes cannot: a wrong assumption about the real
gliner2 API would keep the faked suite green while the engine is broken.
"""

from __future__ import annotations

import os
import subprocess
import sys

import pytest
from elide_core.ner.v1 import (
    EntitySpec,
    Schema,
)

pytestmark = pytest.mark.real


@pytest.fixture(scope="module")
def engine():
    from elide_ner.inference.backends.gliner2 import Engine

    return Engine()


def test_entity_extraction_against_the_real_model(engine):
    schema = Schema(entities=[EntitySpec(label="person"), EntitySpec(label="email")])
    [resp] = engine.recognize(
        ["Ada Lovelace, ada@example.com, writes in English."], schema, threshold=0.3
    )

    labels = {e.label: e for e in resp.entities}
    assert "person" in labels
    assert labels["person"].text == "Ada Lovelace"
    assert (labels["person"].start, labels["person"].end) == (0, 12)
    assert "email" in labels
    assert labels["email"].text == "ada@example.com"


def test_offline_load_and_infer(engine):
    # `engine` fills the cache this loads from. In a fresh process:
    # huggingface_hub reads HF_HUB_OFFLINE when it is first imported, so
    # setting it in this one, after the fixture imported it, would not take
    # effect. The service runs offline, and must load and infer from the cache.
    script = (
        "from elide_core.ner.v1 import EntitySpec, Schema\n"
        "from elide_ner.inference.backends.gliner2 import Engine\n"
        "schema = Schema(entities=[EntitySpec(label='person')])\n"
        "[resp] = Engine().recognize(['Ada in London'], schema, 0.3)\n"
        "assert any(e.label == 'person' for e in resp.entities), resp\n"
    )
    env = {**os.environ, "HF_HUB_OFFLINE": "1"}
    result = subprocess.run([sys.executable, "-c", script], env=env, capture_output=True, text=True)
    assert result.returncode == 0, result.stderr[-2000:]


def test_over_length_rejected(engine):
    from elide_ner.inference.backends.gliner2 import TextTooLongError

    with pytest.raises(TextTooLongError):
        engine.check_length("word " * (engine.max_tokens + 50))
