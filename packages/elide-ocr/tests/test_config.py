"""Configuration snapshots and validation."""

import pytest
from elide_ocr.runtime import config
from pydantic import ValidationError

ENVS = (config.DETECTION_ENV, config.RECOGNITION_ENV, config.DEVICE_ENV, config.BACKEND_ENV)


@pytest.fixture(autouse=True)
def clean_settings(monkeypatch):
    for name in ENVS:
        monkeypatch.delenv(name, raising=False)
    config.get_settings.cache_clear()
    yield
    config.get_settings.cache_clear()


def test_settings_are_read_once_and_frozen(monkeypatch):
    monkeypatch.setenv(config.DETECTION_ENV, "fast_base")
    monkeypatch.setenv(config.BACKEND_ENV, "")
    settings = config.get_settings()
    assert settings.detection == "fast_base"
    assert settings.backend == config.DEFAULT_BACKEND
    monkeypatch.setenv(config.DETECTION_ENV, "db_resnet34")
    assert config.get_settings() is settings
    assert settings.detection == "fast_base"
    with pytest.raises(ValidationError):
        settings.detection = "changed"


PINNED = "Felix92/onnxtr-parseq-multilingual-v1@" + "0" * 40


def test_model_id_joins_the_pair():
    settings = config.OcrSettings(detection="fast_base", recognition="parseq")
    assert settings.model_id == "fast_base+parseq"
    assert config.OcrSettings().model_id == f"db_resnet50+{config.DEFAULT_RECOGNITION}"


def test_the_default_recognizer_is_a_pinned_hub_model():
    """Every built-in recognizer has a French vocabulary; see the config comment."""
    assert config.HUB.fullmatch(config.DEFAULT_RECOGNITION)


def test_a_hub_model_pinned_to_a_commit_is_accepted(monkeypatch):
    monkeypatch.setenv(config.RECOGNITION_ENV, PINNED)
    assert config.OcrSettings().recognition == PINNED


@pytest.mark.parametrize(
    "value",
    [
        "Felix92/onnxtr-parseq-multilingual-v1",  # unpinned: whatever main is today
        "Felix92/onnxtr-parseq-multilingual-v1@main",  # a branch moves
        "Felix92/onnxtr-parseq-multilingual-v1@7d73299",  # a short sha is not a commit
        "Parseq",
        "../weights",
    ],
)
def test_an_unpinned_or_malformed_model_is_rejected(monkeypatch, value):
    monkeypatch.setenv(config.RECOGNITION_ENV, value)
    with pytest.raises(ValidationError, match="pinned to a full commit"):
        config.OcrSettings()


def test_unknown_device_is_rejected(monkeypatch):
    monkeypatch.setenv(config.DEVICE_ENV, "tpu")
    with pytest.raises(ValidationError):
        config.OcrSettings()
