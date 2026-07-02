"""NER wire contract, version 1 — schema-driven information extraction.

A request carries a :class:`Schema` describing what to extract from ``text``:
zero or more **entity** types, **classification** tasks, and **structured**
records. All three are optional and compose in a single request; the response
returns each group's results.

Extracted labels are the model's own (``"person"``, ``"email"``, ``"iban"``, …),
not a shared taxonomy. Mapping those onto a canonical vocabulary is the
*consumer's* job — the runtime owns that map. Scores are confidences in
``[0, 1]``.

The Rust runtime (``nvisy-inference-client``) is the source of truth; these
models mirror it. The wire is camelCase to match the runtime's serde
``rename_all = "camelCase"``. Split across :mod:`~nvisy_core.ner.v1.request` and
:mod:`~nvisy_core.ner.v1.response`; this package re-exports the public surface.
"""

from nvisy_core.ner.v1.request import (
    ClassificationSpec,
    EntitySpec,
    FieldSpec,
    NerRequest,
    Schema,
    StructureSpec,
)
from nvisy_core.ner.v1.response import (
    Classification,
    ClassLabel,
    Entity,
    NerResponse,
    Record,
    Span,
)

__all__ = [
    "ClassLabel",
    "Classification",
    "ClassificationSpec",
    "Entity",
    "EntitySpec",
    "FieldSpec",
    "NerRequest",
    "NerResponse",
    "Record",
    "Schema",
    "Span",
    "StructureSpec",
]
