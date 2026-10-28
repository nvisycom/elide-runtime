"""NER wire contract, version 1 — schema-driven information extraction.

A request carries a :class:`Schema` naming the **entity** types to extract from
``text``, and the response returns the spans found for each.

The schema once also carried classification tasks and structured records, which
GLiNER2 supports. Neither was ever sent — the Rust client serializes entities
only — and their contents reach the model, so they were a way past the schema's
own size bound: one field with five thousand `choices` counted as a single
label and exhausted the service. An unbounded input nothing consumes is not a
feature.

Extracted labels are the model's own (``"person"``, ``"email"``, ``"iban"``, …),
not a shared taxonomy. Mapping those onto a canonical vocabulary is the
*consumer's* job — the runtime owns that map. Scores are confidences in
``[0, 1]``.

These Python models are the source of truth for the NER HTTP contract.
JSON Schema, shared fixtures, and Rust wire types are generated from them.
The wire uses snake_case in Python and Rust. Split across :mod:`~elide_core.ner.v1.request` and
:mod:`~elide_core.ner.v1.response`; this package re-exports the public surface.
"""

from elide_core.ner.v1.request import (
    EntitySpec,
    NerRequest,
    Schema,
)
from elide_core.ner.v1.response import (
    Entity,
    NerResponse,
    Span,
    TokenUsage,
)

__all__ = [
    "Entity",
    "EntitySpec",
    "NerRequest",
    "NerResponse",
    "Schema",
    "Span",
    "TokenUsage",
]
