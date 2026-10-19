"""Tests for the transcription result mapping.

These exercise `project()` and the seconds->milliseconds conversion against
plain dicts, so they need no model weights.
"""

import pytest
from bento_whisper.engine import _speaker_for, _to_ms, project
from bento_whisper.service import DEFAULT_MODEL


def test_to_ms_rounds_not_truncates():
    # 1.9996s must not report 1999ms while the next word starts at 2000ms.
    assert _to_ms(1.9996) == 2000
    assert _to_ms(0.0) == 0
    assert _to_ms(1.5) == 1500


def test_to_ms_handles_absent_and_negative():
    assert _to_ms(None) is None
    # The decoder can emit a small negative start for a leading word.
    assert _to_ms(-0.01) == 0


def test_project_maps_segments_to_milliseconds():
    segments = [{"start": 0.0, "end": 2.4, "text": " Hello there. "}]
    resp = project(segments, "large-v3-turbo", "en", 2400)
    seg = resp.segments[0]
    assert (seg.start_ms, seg.end_ms) == (0, 2400)
    # Text is stripped: Whisper pads segments with a leading space.
    assert seg.text == "Hello there."
    assert seg.language == "en"
    assert resp.model_id == "large-v3-turbo" and resp.duration_ms == 2400


def test_project_drops_segments_without_timing():
    """A segment with no usable span is dropped, not given invented values."""
    segments = [
        {"start": None, "end": 1.0, "text": "no start"},
        {"start": 2.0, "end": 1.0, "text": "inverted"},
        {"start": 0.0, "end": 1.0, "text": "good"},
    ]
    assert [s.text for s in project(segments, "m", None, None).segments] == ["good"]


def test_project_maps_words_with_probability_as_confidence():
    """faster-whisper reports a per-word probability, which is a real [0,1] score."""
    segments = [
        {
            "start": 0.0,
            "end": 1.0,
            "text": "Hi there",
            "words": [
                {"word": " Hi", "start": 0.0, "end": 0.4, "probability": 0.97},
                {"word": " there", "start": 0.4, "end": 1.0, "probability": 0.88},
            ],
        }
    ]
    words = project(segments, "m", None, None).segments[0].words
    assert [w.text for w in words] == ["Hi", "there"]
    assert words[0].end_ms == 400
    assert words[0].confidence == pytest.approx(0.97)


def test_project_drops_words_without_timing():
    """A word the decoder could not place is dropped, not fabricated."""
    segments = [
        {
            "start": 0.0,
            "end": 1.0,
            "text": "Hi",
            "words": [
                {"word": "Hi", "start": 0.0, "end": 0.4, "probability": 0.9},
                {"word": "42"},
            ],
        }
    ]
    assert len(project(segments, "m", None, None).segments[0].words) == 1


def test_no_speaker_when_not_diarized():
    segments = [{"start": 0.0, "end": 1.0, "text": "x"}]
    assert project(segments, "m", None, None).segments[0].speaker_id is None


def test_project_attributes_speaker_by_overlap():
    """A segment straddling a speaker change goes to whoever holds most of it."""
    segments = [{"start": 0.0, "end": 1.0, "text": "x"}]
    turns = [(0.0, 0.2, "SPEAKER_00"), (0.2, 1.0, "SPEAKER_01")]
    assert project(segments, "m", None, None, turns).segments[0].speaker_id == "SPEAKER_01"


def test_speaker_for_ignores_non_overlapping_turns():
    turns = [(5.0, 6.0, "SPEAKER_00")]
    assert _speaker_for(0.0, 1.0, turns) is None
    assert _speaker_for(None, 1.0, turns) is None
    assert _speaker_for(0.0, 1.0, []) is None


def test_speaker_ties_are_deterministic():
    turns = [(0.0, 0.5, "A"), (0.5, 1.0, "B")]
    assert _speaker_for(0.0, 1.0, turns) == "A"


def test_segment_confidence_is_absent():
    """avg_logprob is a log probability, not a [0,1] confidence; it must not leak."""
    segments = [{"start": 0.0, "end": 1.0, "text": "x", "avg_logprob": -0.35}]
    assert project(segments, "m", None, None).segments[0].confidence is None


def test_default_model_is_turbo():
    """The default is large-v3-turbo, not large-v3.

    Pinned deliberately: turbo is 2-5x faster for a small accuracy cost, and
    the choice is documented in the README. A silent revert to large-v3 would
    slow every deployment that does not set ELIDE_BENTO_MODEL_NAME.
    """
    assert DEFAULT_MODEL == "large-v3-turbo"
