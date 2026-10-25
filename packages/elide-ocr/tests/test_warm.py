"""Tests for the OCR cache warmer.

The download is not exercised — it fetches weights over the network. What is
asserted is the validation that would otherwise surface as a confusing failure
from inside docTR.
"""

from __future__ import annotations

import pytest
from elide_ocr import warm


def test_a_model_name_that_is_not_a_pair_is_rejected(monkeypatch, tmp_path):
    """docTR needs a detection and a recognition model. One alone is a
    configuration error worth naming at the point it is read."""
    monkeypatch.setenv("DOCTR_CACHE_DIR", str(tmp_path))
    monkeypatch.setenv("ELIDE_BENTO_MODEL_NAME", "db_resnet50")

    with pytest.raises(SystemExit) as excinfo:
        warm.main()

    assert "det+rec" in str(excinfo.value)


def test_a_missing_cache_dir_is_rejected(monkeypatch):
    """The warmer writes into a mounted volume; without the path it would
    populate a directory inside the container that nothing reads."""
    monkeypatch.delenv("DOCTR_CACHE_DIR", raising=False)
    monkeypatch.setenv("ELIDE_BENTO_MODEL_NAME", "db_resnet50+crnn_vgg16_bn")

    with pytest.raises(SystemExit) as excinfo:
        warm.main()

    assert "DOCTR_CACHE_DIR" in str(excinfo.value)
