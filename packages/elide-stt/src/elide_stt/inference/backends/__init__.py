"""Lazy backend selection and construction.

Each registered module exports Engine implementing the SttBackend protocol.
Model libraries are imported only when constructing the selected engine.
"""

from __future__ import annotations

from importlib import import_module
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from types import ModuleType

    from elide_stt.inference.backends.base import SttBackend
    from elide_stt.runtime.config import SttSettings

# Backend name -> module implementing it, relative to this package.
BACKENDS = {
    "faster_whisper": "faster_whisper",
}


def load(name: str) -> ModuleType:
    """Import the selected backend, failing loudly for an unknown name."""
    module = BACKENDS.get(name)
    if module is None:
        known = ", ".join(sorted(BACKENDS))
        raise RuntimeError(f"unknown STT backend {name!r}; known backends: {known}")
    return import_module(f"{__name__}.{module}")


def create(settings: SttSettings) -> SttBackend:
    """Construct the selected backend without exposing modules to callers."""
    return load(settings.backend).Engine(settings)
