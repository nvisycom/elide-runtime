"""Configuration snapshots and mounted-model precedence."""

import pytest
from elide_ocr.runtime import config
from pydantic import ValidationError


@pytest.fixture(autouse=True)
def clean_settings(monkeypatch):
    for name in ("ELIDE_BENTO_MODEL_NAME", "ELIDE_BENTO_MODEL_PATH", config.BACKEND_ENV):
        monkeypatch.delenv(name, raising=False)
    config.get_settings.cache_clear()
    yield
    config.get_settings.cache_clear()


def test_settings_preserve_environment_names_and_empty_defaults(monkeypatch):
    monkeypatch.setenv("ELIDE_BENTO_MODEL_NAME", "det+rec")
    monkeypatch.setenv(config.BACKEND_ENV, "")
    settings = config.get_settings()
    assert settings.model_name == "det+rec"
    assert settings.backend == config.DEFAULT_BACKEND
    monkeypatch.setenv("ELIDE_BENTO_MODEL_NAME", "other+model")
    assert config.get_settings() is settings
    assert settings.model_name == "det+rec"
    with pytest.raises(ValidationError):
        settings.model_name = "changed"


def test_mounted_weights_override_model_name(tmp_path):
    settings = config.OcrSettings(model_name="det+rec", model_path=str(tmp_path))
    assert settings.resolve_model() == "det+rec"
    (tmp_path / "weights.bin").write_bytes(b"weights")
    assert settings.resolve_model() == str(tmp_path)
