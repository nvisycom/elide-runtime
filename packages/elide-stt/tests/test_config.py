"""Validate transcription settings before loading models."""

import pytest
from elide_stt.runtime import config
from pydantic import ValidationError


@pytest.fixture(autouse=True)
def clean_settings(monkeypatch):
    for name in (
        config.MODEL_ENV,
        config.BACKEND_ENV,
        config.COMPUTE_TYPE_ENV,
        config.DEVICE_ENV,
        config.DIARIZE_ENV,
        config.HF_TOKEN_ENV,
        config.DIARIZE_MODEL_ENV,
        config.ALIGN_ENV,
        config.MAX_DURATION_ENV,
        config.MAX_BYTES_ENV,
        config.MIN_SPEAKERS_ENV,
        config.MAX_SPEAKERS_ENV,
    ):
        monkeypatch.delenv(name, raising=False)
    config.get_settings.cache_clear()
    yield
    config.get_settings.cache_clear()


@pytest.mark.parametrize(
    "name",
    [
        config.MAX_DURATION_ENV,
        config.MAX_BYTES_ENV,
        config.MIN_SPEAKERS_ENV,
        config.MAX_SPEAKERS_ENV,
    ],
)
@pytest.mark.parametrize("value", ["0", "-1", "invalid"])
def test_invalid_limits_rejected(monkeypatch, name, value):
    monkeypatch.setenv(name, value)
    with pytest.raises(ValidationError):
        config.SttSettings()


@pytest.mark.parametrize("name", [config.DIARIZE_ENV, config.ALIGN_ENV])
def test_invalid_boolean_rejected(monkeypatch, name):
    monkeypatch.setenv(name, "maybe")
    with pytest.raises(ValidationError):
        config.SttSettings()


def test_device_compute_defaults_and_empty_values(monkeypatch):
    monkeypatch.setenv(config.COMPUTE_TYPE_ENV, "")
    monkeypatch.setenv(config.ALIGN_ENV, "")
    assert config.SttSettings().resolved_compute_type == "int8"
    assert config.SttSettings().word_timestamps is True
    monkeypatch.setenv(config.DEVICE_ENV, "cuda")
    assert config.SttSettings().resolved_compute_type == "float16"
    monkeypatch.setenv(config.COMPUTE_TYPE_ENV, "int8_float16")
    assert config.SttSettings().resolved_compute_type == "int8_float16"


def test_settings_are_an_immutable_snapshot_and_hide_token(monkeypatch):
    monkeypatch.setenv(config.HF_TOKEN_ENV, "private-token")
    settings = config.get_settings()
    monkeypatch.setenv(config.DEVICE_ENV, "cuda")
    assert config.get_settings() is settings
    assert settings.device == "cpu"
    assert "private-token" not in repr(settings)
    with pytest.raises(ValidationError):
        settings.device = "cuda"


def test_speaker_bounds_must_be_ordered():
    with pytest.raises(ValidationError, match="min_speakers"):
        config.SttSettings(min_speakers=3, max_speakers=2)


def test_model_reads_the_stt_prefixed_variable(monkeypatch):
    assert config.SttSettings().model_id == config.DEFAULT_MODEL
    monkeypatch.setenv(config.MODEL_ENV, "distil-large-v3")
    assert config.SttSettings().model_id == "distil-large-v3"


def test_unknown_device_rejected(monkeypatch):
    monkeypatch.setenv(config.DEVICE_ENV, "tpu")
    with pytest.raises(ValidationError):
        config.SttSettings()
