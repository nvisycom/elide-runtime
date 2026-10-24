"""Model backends for the NER service, selected by ``ELIDE_BENTO_NER_BACKEND``.

The service talks to a backend only through :class:`Engine` and
:class:`TextTooLongError`, so a new model family is a module here plus an entry
in :data:`BACKENDS` — not a new service, image, or wire contract. That is the
point of the split: ``gliner2`` couples to one library's API (``gliner2.Schema``,
``batch_extract``, its result dict), and a successor will couple to a different
one, while the contract in ``elide_core.ner.v1`` stays put.

A backend module must export:

``Engine``
    Loads the model and translates the wire schema both ways. Needs
    ``max_tokens``, ``check_length(text) -> int``, and
    ``recognize(texts, schema, threshold, token_counts) -> list[NerResponse]``.
``TextTooLongError``
    Raised by ``check_length`` when input would be silently truncated.

The import is lazy: naming one backend must not import another's model library,
since each pulls heavy optional dependencies the image may not carry.
"""

from __future__ import annotations

import os
from importlib import import_module
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from types import ModuleType

# Backend name -> module implementing it, relative to this package.
BACKENDS = {
    "gliner2": "gliner2",
}

BACKEND_ENV = "ELIDE_BENTO_NER_BACKEND"
DEFAULT_BACKEND = "gliner2"


def backend_name() -> str:
    return os.getenv(BACKEND_ENV, "").strip() or DEFAULT_BACKEND


def load() -> ModuleType:
    """Import the selected backend module.

    Raises ``RuntimeError`` on an unknown name rather than falling back to the
    default: a deployment that asked for a specific model should fail loudly, not
    quietly serve a different one.
    """
    name = backend_name()
    module = BACKENDS.get(name)
    if module is None:
        known = ", ".join(sorted(BACKENDS))
        raise RuntimeError(f"unknown NER backend {name!r}; known backends: {known}")
    return import_module(f"{__name__}.{module}")
