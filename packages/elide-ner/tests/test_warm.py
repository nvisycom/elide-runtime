"""Tests for the NER cache warmer.

The download is patched — it fetches over a gigabyte. What is asserted is that
the warm reads the model id from the same place the service loads it, so the
two cannot drift apart.
"""

from __future__ import annotations

import pytest
from elide_ner import warm


def test_the_warm_uses_the_model_the_service_loads(monkeypatch, tmp_path):
    monkeypatch.setenv("HF_HOME", str(tmp_path))
    monkeypatch.setenv("ELIDE_BENTO_NER_MODEL", "org/some-ner-model")
    monkeypatch.setattr(warm, "hand_over", lambda cache: None)

    hub = pytest.importorskip("huggingface_hub")
    calls: list[str] = []
    monkeypatch.setattr(hub, "snapshot_download", lambda model: calls.append(model))

    warm.main()

    assert calls == ["org/some-ner-model"]


def test_a_missing_cache_dir_is_rejected(monkeypatch):
    """The warmer writes into a mounted volume; without the path it would
    populate a directory inside the container that nothing reads."""
    monkeypatch.delenv("HF_HOME", raising=False)
    monkeypatch.setenv("ELIDE_BENTO_NER_MODEL", "org/some-ner-model")

    with pytest.raises(SystemExit) as excinfo:
        warm.main()

    assert "HF_HOME" in str(excinfo.value)
