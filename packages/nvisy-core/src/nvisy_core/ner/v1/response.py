"""NER v1 response: extracted spans, classifications, and structured records.

The response returns each schema group's results. Extracted labels are the
model's own (``"person"``, ``"email"``, …), not a shared taxonomy — mapping them
onto a canonical vocabulary is the consumer's job. Scores are confidences in
``[0, 1]``.
"""

from __future__ import annotations

from pydantic import Field, model_validator

from nvisy_core.ner.v1.base import _Model
from nvisy_core.types import Probability


class Span(_Model):
    """A matched substring with offsets and confidence (no label of its own)."""

    text: str = Field(description="The matched substring, text[start:end].")
    score: Probability
    start: int = Field(ge=0, description="Character offset, inclusive.")
    end: int = Field(ge=0, description="Character offset, exclusive.")

    @model_validator(mode="after")
    def _check_span(self) -> Span:
        if self.end <= self.start:
            raise ValueError("end must be greater than start")
        return self


class Entity(_Model):
    """A matched entity span, carrying its model-native label."""

    text: str = Field(description="The matched substring, text[start:end].")
    label: str = Field(description="The model-native entity label (e.g. 'person', 'email').")
    score: Probability
    start: int = Field(ge=0, description="Character offset, inclusive.")
    end: int = Field(ge=0, description="Character offset, exclusive.")

    @model_validator(mode="after")
    def _check_span(self) -> Entity:
        if self.end <= self.start:
            raise ValueError("end must be greater than start")
        return self


class ClassLabel(_Model):
    label: str
    score: Probability


# A classification result: one best label, or a list when multiLabel was set.
Classification = ClassLabel | list[ClassLabel]

# A structured record: each field name maps to its matched spans.
Record = dict[str, list[Span]]


class NerResponse(_Model):
    entities: list[Entity] = Field(default_factory=list)
    classifications: dict[str, Classification] = Field(
        default_factory=dict, description="Keyed by classification task name."
    )
    structures: dict[str, list[Record]] = Field(
        default_factory=dict, description="Keyed by structure name; a list of records each."
    )
    model_id: str = Field(description="Hugging Face id of the GLiNER2 model that produced this.")
