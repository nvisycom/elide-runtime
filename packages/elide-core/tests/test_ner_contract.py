"""Check the Python-owned artifacts also consumed by Rust tests."""

import json
from pathlib import Path

import pytest
from elide_core.ner.v1.contract import NerInput, NerOutput

CONTRACT = Path(__file__).resolve().parents[3] / "contracts/ner/v1"
CASES = json.loads((CONTRACT / "fixtures.json").read_text())


@pytest.mark.parametrize("case", CASES, ids=["ascii", "accents", "emoji-cjk", "combining"])
def test_shared_fixtures_validate_and_preserve_offsets(case):
    request = NerInput.model_validate(case["request"])
    response = NerOutput.model_validate(case["response"])
    assert len(request.requests) == len(response.root)
    text = request.requests[0].text
    entity = response.root[0].entities[0]
    assert text[entity.start : entity.end] == entity.text
    assert case["expected_byte_range"] == [
        len(text[: entity.start].encode()),
        len(text[: entity.end].encode()),
    ]
