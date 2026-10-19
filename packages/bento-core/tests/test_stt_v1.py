"""Tests for the STT wire contract."""

import pytest
from bento_core.stt.v1 import Segment, SttRequest, SttResponse, Word


def test_request_parses_camelcase():
    req = SttRequest.model_validate({"audio": "AAAA", "filename": "a.wav", "language": "en"})
    assert req.audio == "AAAA"
    assert req.filename == "a.wav"


def test_request_defaults_hints_to_none():
    req = SttRequest.model_validate({"audio": "AAAA"})
    assert req.filename is None and req.language is None


def test_response_serializes_camelcase():
    resp = SttResponse(
        segments=[
            Segment(
                start_ms=0,
                end_ms=2400,
                text="Hello there.",
                speaker_id="SPEAKER_00",
                language="en",
                confidence=0.94,
                words=[Word(start_ms=0, end_ms=480, text="Hello", confidence=0.97)],
            )
        ],
        model_id="large-v3-turbo",
        language="en",
        duration_ms=2400,
    )
    # No `by_alias=True`: that would override the model's own
    # `serialize_by_alias`, so the test would pass even if the config were
    # wrong. The wire form has to come from the config alone.
    dumped = resp.model_dump(mode="json")
    assert dumped["segments"][0]["startMs"] == 0
    assert dumped["segments"][0]["speakerId"] == "SPEAKER_00"
    assert dumped["segments"][0]["words"][0]["endMs"] == 480
    assert dumped["modelId"] == "large-v3-turbo"
    assert dumped["durationMs"] == 2400


def test_optional_fields_omitted_are_none():
    """A non-diarizing, non-aligning deployment still produces a valid response."""
    resp = SttResponse(
        segments=[Segment(start_ms=0, end_ms=100, text="hi")], model_id="large-v3-turbo"
    )
    seg = resp.model_dump(by_alias=True, mode="json")["segments"][0]
    assert seg["speakerId"] is None
    assert seg["confidence"] is None
    assert seg["words"] == []


def test_segment_rejects_inverted_span():
    with pytest.raises(ValueError, match="greater than or equal to"):
        Segment(start_ms=500, end_ms=100, text="x")


def test_word_rejects_inverted_span():
    with pytest.raises(ValueError, match="greater than or equal to"):
        Word(start_ms=500, end_ms=100, text="x")


def test_zero_length_span_allowed():
    """A zero-length span is legal: alignment can place a word at a point."""
    assert Word(start_ms=100, end_ms=100, text="x").end_ms == 100


def test_confidence_bounded():
    with pytest.raises(ValueError):
        Word(start_ms=0, end_ms=1, text="x", confidence=1.5)
