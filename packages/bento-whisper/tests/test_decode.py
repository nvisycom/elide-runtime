"""Tests for the audio ingress path.

These cover `_decode_audio`'s guards, which run before any model is touched,
so they need no weights.
"""

import base64
import io
import struct
import wave

import pytest
from bento_whisper.service import SAMPLE_RATE, _decode_audio
from bentoml.exceptions import InvalidArgument


def _wav_b64(seconds: float, rate: int = SAMPLE_RATE, channels: int = 1) -> str:
    """A base64 silent WAV of the given length, rate and channel count."""
    buf = io.BytesIO()
    with wave.open(buf, "wb") as w:
        w.setnchannels(channels)
        w.setsampwidth(2)
        w.setframerate(rate)
        samples = int(seconds * rate) * channels
        w.writeframes(struct.pack(f"<{samples}h", *([0] * samples)))
    return base64.b64encode(buf.getvalue()).decode()


def test_decodes_wav_to_waveform():
    waveform = _decode_audio(_wav_b64(0.5))
    # Half a second at the target rate, within a resampling frame or two.
    assert abs(len(waveform) - SAMPLE_RATE // 2) < 100


def test_rejects_non_base64():
    with pytest.raises(InvalidArgument, match="valid base64"):
        _decode_audio("not base64!!")


def test_rejects_empty_payload():
    with pytest.raises(InvalidArgument, match="empty"):
        _decode_audio("")


def test_rejects_undecodable_bytes():
    with pytest.raises(InvalidArgument, match="not decodable"):
        _decode_audio(base64.b64encode(b"this is not audio").decode())


def test_oversized_payload_rejected_before_decode():
    """The size cap refuses an oversized body without decoding it."""
    with pytest.raises(InvalidArgument, match="base64 characters"):
        _decode_audio(_wav_b64(1.0), max_bytes=1024)


def test_decode_is_bounded_by_max_seconds():
    """Audio longer than the bound is truncated, not decoded whole.

    The input must actually exceed `max_seconds`, otherwise the test would
    pass even if the duration argument were ignored entirely.
    """
    waveform = _decode_audio(_wav_b64(3.0), max_seconds=1.0)
    # One second decoded, not three.
    assert abs(len(waveform) - SAMPLE_RATE) < 100


def test_valid_high_rate_stereo_is_not_rejected():
    """A 48 kHz stereo source is accepted and resampled to mono 16 kHz.

    Regression: deriving the byte cap from the *output* PCM format rejected
    valid audio, since a 48 kHz stereo WAV is six times the size of the
    16 kHz mono waveform it decodes to.
    """
    waveform = _decode_audio(
        _wav_b64(2.0, rate=48_000, channels=2), max_seconds=61, max_bytes=512 * 1024 * 1024
    )
    assert abs(len(waveform) - 2 * SAMPLE_RATE) < 100
