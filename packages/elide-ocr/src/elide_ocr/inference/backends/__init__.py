"""Lazy backend selection and construction.

Each registered module exports Engine implementing the OcrBackend protocol.
Model libraries are imported only when constructing the selected engine.
"""

from __future__ import annotations

from importlib import import_module
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from types import ModuleType

    from elide_ocr.inference.backends.base import OcrBackend
    from elide_ocr.runtime.config import OcrSettings

# Backend name -> module implementing it, relative to this package.
BACKENDS = {
    "onnxtr": "onnxtr",
}


def load(name: str) -> ModuleType:
    """Import the selected backend, failing loudly for an unknown name."""
    module = BACKENDS.get(name)
    if module is None:
        known = ", ".join(sorted(BACKENDS))
        raise RuntimeError(f"unknown OCR backend {name!r}; known backends: {known}")
    return import_module(f"{__name__}.{module}")


def create(settings: OcrSettings) -> OcrBackend:
    """Construct the selected backend without exposing modules to callers."""
    return load(settings.backend).Engine(settings)
