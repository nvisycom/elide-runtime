"""Shared wire-contract types for the Nvisy inference services.

The contract is task-named and versioned:

- :mod:`elide_core.ocr` — OCR request/response (``Page → Block → Line → Word``).
- :mod:`elide_core.ner` — NER request/response (entities with model-native
  labels; the consumer owns the taxonomy mapping).

The Python models own the HTTP contracts and generate the Rust wire types in
``elide-bentoml``. Rust domain types and taxonomy mapping remain independent.
"""

from elide_core import ner, ocr

__all__ = ["ner", "ocr"]
