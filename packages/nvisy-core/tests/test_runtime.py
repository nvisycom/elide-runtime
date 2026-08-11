"""Tests for the shared runtime helpers."""

import pytest
from nvisy_core.runtime import request_id, resolve_model


def test_populated_path_takes_precedence(monkeypatch, tmp_path):
    (tmp_path / "weights.bin").write_text("x")  # non-empty dir = mounted weights
    monkeypatch.setenv("NVISY_MODEL_PATH", str(tmp_path))
    monkeypatch.setenv("NVISY_MODEL_NAME", "ignored/name")
    assert resolve_model() == str(tmp_path)


def test_empty_path_falls_through_to_name(monkeypatch, tmp_path):
    # An empty /models mount means "no BYO weights" -> use the named model.
    monkeypatch.setenv("NVISY_MODEL_PATH", str(tmp_path))  # exists but empty
    monkeypatch.setenv("NVISY_MODEL_NAME", "org/some-model")
    assert resolve_model() == "org/some-model"


def test_missing_path_uses_name(monkeypatch):
    monkeypatch.setenv("NVISY_MODEL_PATH", "/does/not/exist")
    monkeypatch.setenv("NVISY_MODEL_NAME", "org/some-model")
    assert resolve_model() == "org/some-model"


def test_raises_when_no_name_and_no_weights(monkeypatch):
    # Served bentos inject NVISY_MODEL_NAME; without it (and no mounted weights),
    # resolve_model has nothing to load and raises rather than guess.
    monkeypatch.delenv("NVISY_MODEL_PATH", raising=False)
    monkeypatch.delenv("NVISY_MODEL_NAME", raising=False)
    with pytest.raises(RuntimeError):
        resolve_model()


def test_request_id_none_without_headers():
    assert request_id(object()) is None


def test_request_id_reads_header():
    class _Ctx:
        request = type("R", (), {"headers": {"x-request-id": "abc-123"}})()

    assert request_id(_Ctx()) == "abc-123"
