"""NER v1 request: the extraction schema and the request envelope.

A request carries a :class:`Schema` describing what to extract from ``text``:
zero or more **entity** types, **classification** tasks, and **structured**
records. All three are optional and compose in a single request.
"""

from __future__ import annotations

from typing import Literal

from pydantic import Field, model_validator

from nvisy_core.ner.v1.base import _Model
from nvisy_core.types import Probability


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


class ClassificationSpec(_Model):
    """A text-classification task: assign ``text`` to one (or more) of ``labels``."""

    task: str = Field(min_length=1, description="Name of the task; keys the result.")
    labels: list[str] = Field(min_length=1)
    multi_label: bool = Field(
        default=False, description="Allow multiple labels (vs. single best label)."
    )
    threshold: Probability | None = Field(default=None)


class FieldSpec(_Model):
    """A field of a structured record."""

    name: str = Field(min_length=1)
    dtype: Literal["str", "list"] = Field(
        default="list", description="Single value ('str') or a list of values ('list')."
    )
    choices: list[str] | None = Field(
        default=None, description="Constrain the field to an enum of values."
    )
    description: str | None = None
    threshold: Probability | None = None
    pattern: str | None = Field(
        default=None, description="Regex the field value must match (compiled to a validator)."
    )


class StructureSpec(_Model):
    """A named structured record made of fields."""

    name: str = Field(min_length=1, description="Name of the record; keys the result.")
    fields: list[FieldSpec] = Field(min_length=1)

    @model_validator(mode="after")
    def _unique_fields(self) -> StructureSpec:
        names = [f.name for f in self.fields]
        if len(names) != len(set(names)):
            raise ValueError(f"structure {self.name!r} has duplicate field names")
        return self


class Schema(_Model):
    """What to extract. At least one group must be non-empty."""

    entities: list[EntitySpec] = Field(default_factory=list)
    classifications: list[ClassificationSpec] = Field(default_factory=list)
    structures: list[StructureSpec] = Field(default_factory=list)

    @model_validator(mode="after")
    def _non_empty_and_unique(self) -> Schema:
        if not (self.entities or self.classifications or self.structures):
            raise ValueError(
                "schema must declare at least one entity, classification, or structure"
            )
        _reject_dupes("entity labels", [e.label for e in self.entities])
        _reject_dupes("classification tasks", [c.task for c in self.classifications])
        _reject_dupes("structure names", [s.name for s in self.structures])
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
