"""Environment parsing and startup configuration consistency."""

import pytest
from elide_ner.runtime import config
from pydantic import ValidationError


@pytest.fixture(autouse=True)
def clean_settings(monkeypatch):
    for name in (
        config.MODEL_ENV,
        config.BACKEND_ENV,
        config.QUANTIZE_ENV,
        config.COMPILE_ENV,
        config.MAX_TOKENS_ENV,
        config.MAX_SCHEMA_BYTES_ENV,
    ):
        monkeypatch.delenv(name, raising=False)
    config.get_settings.cache_clear()
    yield
    config.get_settings.cache_clear()


@pytest.mark.parametrize("field", ["max_tokens", "max_schema_bytes"])
@pytest.mark.parametrize("value", ["0", "-1", "invalid"])
def test_invalid_limits_are_rejected(monkeypatch, field, value):
    monkeypatch.setenv(f"ELIDE_BENTO_NER_{field.upper()}", value)
    with pytest.raises(ValidationError):
        config.NerSettings()


@pytest.mark.parametrize("name", [config.QUANTIZE_ENV, config.COMPILE_ENV])
def test_invalid_boolean_is_rejected(monkeypatch, name):
    monkeypatch.setenv(name, "maybe")
    with pytest.raises(ValidationError):
        config.NerSettings()


def test_empty_environment_values_use_defaults(monkeypatch):
    for name in (config.MODEL_ENV, config.BACKEND_ENV, config.QUANTIZE_ENV, config.COMPILE_ENV):
        monkeypatch.setenv(name, "")
    settings = config.NerSettings()
    assert settings.model_id == config.DEFAULT_MODEL
    assert settings.backend == config.DEFAULT_BACKEND
    assert settings.quantize is False
    assert settings.compile_model is False


def test_process_configuration_is_a_single_immutable_snapshot(monkeypatch):
    monkeypatch.setenv(config.MAX_TOKENS_ENV, "256")
    settings = config.get_settings()
    monkeypatch.setenv(config.MAX_TOKENS_ENV, "128")
    assert config.get_settings() is settings
    assert settings.max_tokens == 256
    with pytest.raises(ValidationError):
        settings.max_tokens = 128


def test_explicit_settings_override_environment(monkeypatch):
    monkeypatch.setenv(config.MODEL_ENV, "org/environment")
    monkeypatch.setenv(config.COMPILE_ENV, "false")
    settings = config.NerSettings(model_id="org/explicit", compile_model=True)
    assert settings.model_id == "org/explicit"
    assert settings.compile_model is True
