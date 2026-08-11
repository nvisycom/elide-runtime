"""Shared wire-contract types for the Nvisy inference services.

The contract is task-named and versioned:

- :mod:`nvisy_core.ocr` — OCR request/response (``Page → Block → Line → Word``).
- :mod:`nvisy_core.ner` — NER request/response (entities with model-native
  labels; the consumer owns the taxonomy mapping).

These mirror the Rust runtime (``nvisy-inference-client`` / ``nvisy-ontology``),
which is the source of truth; versioning is lockstep with the runtime.
"""

from nvisy_core import ner, ocr

__all__ = ["ner", "ocr"]
