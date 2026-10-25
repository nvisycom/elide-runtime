"""Tests for the STT cache warmer.

The transcription download is patched — it fetches gigabytes. What is asserted
is the gated-model guard, which is the part a deployment gets wrong.
"""

from __future__ import annotations

import pytest
from elide_stt import warm


def test_diarization_without_a_token_is_refused(monkeypatch, tmp_path):
    """The diarization model is gated, so enabling it without a token cannot
    work. Failing here names the reason; failing later surfaces as a 401 from
    inside the service's startup."""
    monkeypatch.setenv("HF_HOME", str(tmp_path))
    monkeypatch.setenv("ELIDE_BENTO_MODEL_NAME", "large-v3-turbo")
    monkeypatch.setenv("ELIDE_BENTO_STT_DIARIZE", "true")
    monkeypatch.setenv("ELIDE_BENTO_STT_DIARIZE_MODEL", "pyannote/some-model")
    monkeypatch.setenv("ELIDE_BENTO_STT_HF_TOKEN", "")

    fw = pytest.importorskip("faster_whisper.utils")
    monkeypatch.setattr(fw, "download_model", lambda model: None)

    with pytest.raises(SystemExit) as excinfo:
        warm.main()

    message = str(excinfo.value)
    assert "ELIDE_BENTO_STT_HF_TOKEN" in message
    assert "pyannote/some-model" in message, "the message should link the gate"


def test_diarization_off_never_requests_the_gated_model(monkeypatch, tmp_path):
    """Off by default, and off means the gated model is never fetched."""
    monkeypatch.setenv("HF_HOME", str(tmp_path))
    monkeypatch.setenv("ELIDE_BENTO_MODEL_NAME", "large-v3-turbo")
    monkeypatch.delenv("ELIDE_BENTO_STT_DIARIZE", raising=False)
    monkeypatch.setattr(warm, "hand_over", lambda cache: None)

    fw = pytest.importorskip("faster_whisper.utils")
    downloaded: list[str] = []
    monkeypatch.setattr(fw, "download_model", lambda model: downloaded.append(model))

    warm.main()

    assert downloaded == ["large-v3-turbo"]


def test_a_missing_token_is_caught_before_any_download(monkeypatch, tmp_path):
    """Validate the configuration before spending a gigabyte on it.

    The transcription model downloads first and is over a gigabyte, so a
    deployment that enabled diarization without a token used to pay for that
    download before being told about a variable readable from the start.
    """
    monkeypatch.setenv("HF_HOME", str(tmp_path))
    monkeypatch.setenv("ELIDE_BENTO_MODEL_NAME", "large-v3-turbo")
    monkeypatch.setenv("ELIDE_BENTO_STT_DIARIZE", "true")
    monkeypatch.setenv("ELIDE_BENTO_STT_DIARIZE_MODEL", "pyannote/some-model")
    monkeypatch.setenv("ELIDE_BENTO_STT_HF_TOKEN", "")

    fw = pytest.importorskip("faster_whisper.utils")
    monkeypatch.setattr(
        fw,
        "download_model",
        lambda model: pytest.fail("the token must be checked before downloading"),
    )

    with pytest.raises(SystemExit):
        warm.main()
