"""Lazy backend selection and construction.

Each registered module exports Engine implementing the NerBackend protocol.
Model libraries are imported only when constructing the selected engine.
"""

from __future__ import annotations

from importlib import import_module
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from types import ModuleType

    from elide_ner.inference.backends.base import NerBackend
    from elide_ner.runtime.config import NerSettings

# Backend name -> module implementing it, relative to this package.
BACKENDS = {
    "gliner2": "gliner2",
}


def load(name: str) -> ModuleType:
    """Import the selected backend, failing loudly for an unknown name."""
    module = BACKENDS.get(name)
    if module is None:
        known = ", ".join(sorted(BACKENDS))
        raise RuntimeError(f"unknown NER backend {name!r}; known backends: {known}")
    return import_module(f"{__name__}.{module}")


def create(settings: NerSettings) -> NerBackend:
    """Construct the selected backend without exposing modules to callers."""
    return load(settings.backend).Engine(settings)
