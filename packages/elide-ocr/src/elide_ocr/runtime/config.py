"""Configuration defaults for the OCR service."""

import re
from functools import lru_cache
from typing import Annotated, Literal

from pydantic import AfterValidator, Field
from pydantic_settings import BaseSettings, SettingsConfigDict

# A model is named one of two ways, and both pin the weights:
#
# - A built-in OnnxTR architecture ("db_resnet50"). Its weights download from
#   OnnxTR's release assets, checked against a SHA-256 OnnxTR names per
#   architecture, so the lockfile's onnxtr version is what pins them.
# - A Hugging Face Hub model, "<owner>/<repo>@<commit>", with the full 40-hex
#   commit. A branch or tag can move under a deployment; a commit cannot.
BUILTIN = re.compile(r"[a-z0-9_]+")
HUB = re.compile(r"(?P<repo>[\w.-]+/[\w.-]+)@(?P<revision>[0-9a-f]{40})")


def _model_ref(value: str) -> str:
    if BUILTIN.fullmatch(value) or HUB.fullmatch(value):
        return value
    raise ValueError(
        "expected a built-in OnnxTR architecture such as 'db_resnet50', or a Hub model "
        "pinned to a full commit: '<owner>/<repo>@<40-hex sha>'"
    )


ModelRef = Annotated[str, AfterValidator(_model_ref)]

DETECTION_ENV = "ELIDE_BENTO_OCR_DETECTION"
DEFAULT_DETECTION = "db_resnet50"

# docTR's multilingual parseq, published by OnnxTR's maintainer. Every built-in
# recognizer has a French vocabulary, without á, ä, ł or Ł, so a name like
# García, Schäfer or Łukasz cannot be read at all, and what OCR cannot read
# cannot be redacted. This one covers twelve European Latin-script languages
# (Apache-2.0); it has no Cyrillic. See docs/design/ocr.md for the evaluation.
RECOGNITION_ENV = "ELIDE_BENTO_OCR_RECOGNITION"
DEFAULT_RECOGNITION = (
    "Felix92/onnxtr-parseq-multilingual-v1@7d7329948dbcff02637a3eb9c14ae0c4440c5ed0"
)

# The ONNX Runtime execution provider. "cuda" needs an image built with
# onnxruntime-gpu; the published image carries the CPU runtime only.
DEVICE_ENV = "ELIDE_BENTO_OCR_DEVICE"
DEFAULT_DEVICE = "cpu"

BACKEND_ENV = "ELIDE_BENTO_OCR_BACKEND"
DEFAULT_BACKEND = "onnxtr"


class OcrSettings(BaseSettings):
    """Immutable startup configuration."""

    model_config = SettingsConfigDict(
        env_prefix="ELIDE_BENTO_OCR_", env_ignore_empty=True, frozen=True, populate_by_name=True
    )

    detection: ModelRef = DEFAULT_DETECTION
    recognition: ModelRef = DEFAULT_RECOGNITION
    device: Literal["cpu", "cuda"] = DEFAULT_DEVICE
    backend: str = Field(DEFAULT_BACKEND, min_length=1)

    @property
    def model_id(self) -> str:
        """The pair as reported in responses and logs."""
        return f"{self.detection}+{self.recognition}"


@lru_cache(maxsize=1)
def get_settings() -> OcrSettings:
    """Read configuration once per process, outside package import."""
    return OcrSettings()
