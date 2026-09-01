"""Shared wire-contract types for the Nvisy inference services.

The contract is task-named and versioned:

- :mod:`bento_core.ocr` — OCR request/response (``Page → Block → Line → Word``).
- :mod:`bento_core.ner` — NER request/response (entities with model-native
  labels; the consumer owns the taxonomy mapping).

These mirror the Rust client (``elide-bentoml``) in this repo and the runtime's
``nvisy-schema`` (which owns the model-label → shared-taxonomy map);
versioning is lockstep with the runtime.
"""

from bento_core import ner, ocr

__all__ = ["ner", "ocr"]
