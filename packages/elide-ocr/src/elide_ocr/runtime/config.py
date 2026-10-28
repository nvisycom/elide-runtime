"""Configuration defaults for the OCR service."""

from functools import lru_cache
from pathlib import Path

from pydantic import Field
from pydantic_settings import BaseSettings, SettingsConfigDict

# Built-in default detection + recognition models, joined as "det+rec".
# Used as the ELIDE_BENTO_MODEL_NAME default in the service manifest.
DEFAULT_MODEL = "db_resnet50+crnn_vgg16_bn"

BACKEND_ENV = "ELIDE_BENTO_OCR_BACKEND"
DEFAULT_BACKEND = "doctr"


class OcrSettings(BaseSettings):
    """Immutable startup configuration, preserving deployment environment names."""

    model_config = SettingsConfigDict(
        env_prefix="ELIDE_BENTO_OCR_", env_ignore_empty=True, frozen=True, populate_by_name=True
    )

    model_name: str = Field(DEFAULT_MODEL, validation_alias="ELIDE_BENTO_MODEL_NAME", min_length=1)
    model_path: str = Field("/models", validation_alias="ELIDE_BENTO_MODEL_PATH")
    backend: str = Field(DEFAULT_BACKEND, min_length=1)

    def resolve_model(self) -> str:
        """Mounted weights take precedence over the configured model name."""
        weights = Path(self.model_path)
        if self.model_path and weights.is_dir() and any(weights.iterdir()):
            return self.model_path
        return self.model_name


@lru_cache(maxsize=1)
def get_settings() -> OcrSettings:
    """Read configuration once per process, outside package import."""
    return OcrSettings()
