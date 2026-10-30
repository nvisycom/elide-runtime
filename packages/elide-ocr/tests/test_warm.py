"""Tests for the OCR cache warmer.

The download is not exercised — it fetches weights over the network. What is
asserted is the validation that would otherwise surface as a confusing failure,
and that the warmer builds the same engine the service does.
"""

from __future__ import annotations

import pytest
from elide_ocr import warm


@pytest.mark.parametrize("missing", ["ONNXTR_CACHE_DIR", "HF_HOME"])
def test_a_missing_cache_dir_is_rejected(monkeypatch, tmp_path, missing):
    """The warmer writes into a mounted volume; without the path it would
    populate a directory inside the container that nothing reads."""
    monkeypatch.setenv("ONNXTR_CACHE_DIR", str(tmp_path / "onnxtr"))
    monkeypatch.setenv("HF_HOME", str(tmp_path / "huggingface"))
    monkeypatch.delenv(missing)

    with pytest.raises(SystemExit) as excinfo:
        warm.main()

    assert missing in str(excinfo.value)


def test_warmer_builds_the_service_engine_and_hands_over_both_caches(monkeypatch, tmp_path, caplog):
    monkeypatch.setenv("ONNXTR_CACHE_DIR", str(tmp_path / "onnxtr"))
    monkeypatch.setenv("HF_HOME", str(tmp_path / "huggingface"))
    monkeypatch.setenv("ELIDE_BENTO_OCR_DETECTION", "fast_base")
    monkeypatch.setenv("ELIDE_BENTO_OCR_RECOGNITION", "parseq")
    handed = []
    monkeypatch.setattr(warm, "hand_over", handed.append)
    built = []
    monkeypatch.setattr(warm.backends, "create", lambda settings: built.append(settings))
    caplog.set_level("INFO", logger="nvisy.ocr.warm")
    warm.main()
    assert [(s.detection, s.recognition) for s in built] == [("fast_base", "parseq")]
    # A pair of built-ins writes nothing to the Hub cache; it is created anyway,
    # so the hand-over has a directory to give the service.
    assert handed == [str(tmp_path / "onnxtr"), str(tmp_path / "huggingface")]
    assert (tmp_path / "huggingface").is_dir()
    assert "warming fast_base+parseq" in caplog.text
    assert caplog.records[-1].message == "warm"
