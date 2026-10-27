"""NER v1 request: the extraction schema and the request envelope.

A request carries a :class:`Schema` describing what to extract from ``text``:
zero or more **entity** types, **classification** tasks, and **structured**
records. All three are optional and compose in a single request.
"""

from __future__ import annotations

from pydantic import Field, model_validator

from elide_core.ner.v1.base import _Model
from elide_core.types import Probability


class EntitySpec(_Model):
    """An entity type to extract. Zero-shot: a bare label works; a description
    steers the model toward the intended sense."""

    label: str = Field(min_length=1)
    description: str | None = Field(
        default=None,
        description="Optional natural-language description to steer extraction.",
    )
    threshold: Probability | None = Field(
        default=None, description="Per-label confidence cutoff (overrides the request default)."
    )


class Schema(_Model):
    """What to extract: entity labels, at least one.

    GLiNER2 also classifies whole texts and extracts typed records, and this
    schema carried both for a while. Neither was ever sent — the Rust client
    serializes entities only and its decoder discards the other two groups —
    and carrying them cost more than the capability was worth: their contents
    reach the model (a field's `choices` and `description` are embedded), so
    they were a way past the schema's own size bound. One structure with one
    field and five thousand choices is a 170 KB request that counts as a
    single label and kills the worker.

    An unbounded input nothing consumes is not a feature, so they are gone.
    Adding them back means bounding the schema by serialized size rather than
    by counting its groups, which is the bound that does not depend on
    remembering every place text can hide.
    """

    entities: list[EntitySpec] = Field(min_length=1)

    @model_validator(mode="after")
    def _unique_labels(self) -> Schema:
        _reject_dupes("entity labels", [e.label for e in self.entities])
        return self


class NerRequest(_Model):
    text: str = Field(min_length=1)
    schema_: Schema = Field(alias="schema", description="What to extract from the text.")
    threshold: Probability = Field(
        default=0.5,
        description="Default minimum confidence; per-spec thresholds override it.",
    )


def _reject_dupes(what: str, values: list[str]) -> None:
    if len(values) != len(set(values)):
        raise ValueError(f"duplicate {what} in schema")
