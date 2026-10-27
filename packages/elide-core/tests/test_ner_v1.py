"""Tests for the schema-driven NER wire contract."""

import pytest
from elide_core.ner.v1 import (
    Entity,
    NerRequest,
    NerResponse,
    TokenUsage,
)


def test_request_parses_camelcase_schema():
    req = NerRequest.model_validate(
        {
            "text": "Ada in London",
            "schema": {
                "entities": [{"label": "person"}, {"label": "location", "description": "a place"}],
            },
            "threshold": 0.4,
        }
    )
    assert req.text == "Ada in London"
    assert [e.label for e in req.schema_.entities] == ["person", "location"]
    assert req.schema_.entities[1].description == "a place"


def test_empty_schema_rejected():
    """Entities are the only group, so an empty schema has nothing to extract."""
    with pytest.raises(ValueError):
        NerRequest.model_validate({"text": "x", "schema": {}})


def test_duplicate_entity_labels_rejected():
    with pytest.raises(ValueError, match="duplicate entity labels"):
        NerRequest.model_validate(
            {"text": "x", "schema": {"entities": [{"label": "a"}, {"label": "a"}]}}
        )


def test_empty_text_rejected():
    with pytest.raises(ValueError):
        NerRequest.model_validate({"text": "", "schema": {"entities": [{"label": "a"}]}})


@pytest.mark.parametrize(
    ("start", "end"),
    [(5, 5), (5, 4)],
    ids=["empty span", "inverted span"],
)
def test_entity_rejects_a_non_positive_span(start, end):
    """`end <= start` has two cases, and an empty span is the easier to write
    by accident: a zero-width match is not a match, and an inverted one is a
    coordinate bug that would silently index backwards downstream."""
    with pytest.raises(ValueError, match="end must be greater than start"):
        Entity(text="x", label="person", score=0.9, start=start, end=end)


def test_response_serializes_entities_in_camelcase():
    """The wire form is camelCase; entity spans and scores must survive it."""
    resp = NerResponse(
        entities=[Entity(text="Ada", label="person", score=0.9, start=0, end=3)],
        model_id="m",
    )

    dumped = resp.model_dump(by_alias=True)

    assert dumped["entities"][0] == {
        "text": "Ada",
        "label": "person",
        "score": 0.9,
        "start": 0,
        "end": 3,
    }
    assert dumped["modelId"] == "m"


def test_response_omits_tokens_when_absent():
    """`tokens` is optional: a response without it serializes to null."""
    resp = NerResponse(model_id="fastino/x")
    assert resp.tokens is None
    assert resp.model_dump(by_alias=True, mode="json")["tokens"] is None


def test_response_serializes_token_usage():
    """Reported tokens round-trip, with the camelCase wire shape."""
    resp = NerResponse(model_id="fastino/x", tokens=TokenUsage(input=12, limit=512))
    dumped = resp.model_dump(by_alias=True, mode="json")
    assert dumped["tokens"] == {"input": 12, "limit": 512}
    assert NerResponse.model_validate(dumped).tokens.input == 12


def test_token_usage_rejects_negative():
    """Counts are non-negative."""
    with pytest.raises(ValueError, match="greater than or equal to 0"):
        TokenUsage(input=-1, limit=512)
