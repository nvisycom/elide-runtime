"""Tests for the schema-driven NER wire contract."""

import pytest
from bento_core.ner.v1 import (
    ClassLabel,
    Entity,
    NerRequest,
    NerResponse,
    Span,
    TokenUsage,
)


def test_request_parses_camelcase_schema():
    req = NerRequest.model_validate(
        {
            "text": "Ada in London",
            "schema": {
                "entities": [{"label": "person"}, {"label": "location", "description": "a place"}],
                "classifications": [{"task": "lang", "labels": ["en", "fr"], "multiLabel": True}],
                "structures": [{"name": "contact", "fields": [{"name": "email", "pattern": "x"}]}],
            },
            "threshold": 0.4,
        }
    )
    assert req.text == "Ada in London"
    assert [e.label for e in req.schema_.entities] == ["person", "location"]
    assert req.schema_.entities[1].description == "a place"
    assert req.schema_.classifications[0].multi_label is True
    assert req.schema_.structures[0].fields[0].pattern == "x"


def test_empty_schema_rejected():
    with pytest.raises(ValueError, match="at least one"):
        NerRequest.model_validate({"text": "x", "schema": {}})


def test_duplicate_entity_labels_rejected():
    with pytest.raises(ValueError, match="duplicate entity labels"):
        NerRequest.model_validate(
            {"text": "x", "schema": {"entities": [{"label": "a"}, {"label": "a"}]}}
        )


def test_duplicate_structure_field_rejected():
    with pytest.raises(ValueError, match="duplicate field"):
        NerRequest.model_validate(
            {
                "text": "x",
                "schema": {"structures": [{"name": "r", "fields": [{"name": "f"}, {"name": "f"}]}]},
            }
        )


def test_empty_text_rejected():
    with pytest.raises(ValueError):
        NerRequest.model_validate({"text": "", "schema": {"entities": [{"label": "a"}]}})


def test_entity_rejects_non_positive_span():
    with pytest.raises(ValueError, match="end must be greater than start"):
        Entity(text="", label="person", score=0.9, start=3, end=3)


def test_response_serializes_camelcase_groups():
    resp = NerResponse(
        entities=[Entity(text="Ada", label="person", score=0.9, start=0, end=3)],
        classifications={"lang": [ClassLabel(label="en", score=0.8)]},
        structures={"contact": [{"email": [Span(text="a@b.c", score=0.9, start=4, end=9)]}]},
        model_id="fastino/x",
    )
    dumped = resp.model_dump(by_alias=True, mode="json")
    assert dumped["entities"][0]["label"] == "person"
    assert dumped["modelId"] == "fastino/x"
    # multiLabel classification serializes as a list
    assert isinstance(dumped["classifications"]["lang"], list)
    assert dumped["structures"]["contact"][0]["email"][0]["text"] == "a@b.c"


def test_single_label_classification_is_object():
    resp = NerResponse(
        classifications={"sentiment": ClassLabel(label="pos", score=0.8)},
        model_id="fastino/x",
    )
    dumped = resp.model_dump(by_alias=True, mode="json")
    assert dumped["classifications"]["sentiment"] == {"label": "pos", "score": 0.8}


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
