"""NER v1 response: the extracted entity spans.

Extracted labels are the
model's own (``"person"``, ``"email"``, …), not a shared taxonomy — mapping them
onto a canonical vocabulary is the consumer's job. Scores are confidences in
``[0, 1]``.

A response also carries the encoder :class:`TokenUsage` the call spent, when the
service reports it.
"""

from __future__ import annotations

from pydantic import Field, model_validator

from elide_core.ner.v1.base import _Model
from elide_core.types import Probability


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


class TokenUsage(_Model):
    """Encoder tokens the call spent, as the model's own tokenizer counts them.

    ``input`` only: GLiNER2 is an encoder — it scores spans over the input and
    generates nothing, so there is no completion count. The service already
    tokenizes every input to enforce its limit, so reporting this costs no
    extra work.

    ``limit`` is the model's maximum, carried alongside so a consumer can see
    headroom (``input`` of ``limit``) without knowing the deployment's config.
    """

    input: int = Field(ge=0, description="Encoder tokens in the request text.")
    limit: int = Field(ge=0, description="The model's token limit for one input.")


class NerResponse(_Model):
    entities: list[Entity] = Field(default_factory=list)
    model_id: str = Field(description="Hugging Face id of the GLiNER2 model that produced this.")
    tokens: TokenUsage | None = Field(
        default=None,
        description="Encoder tokens the call spent; omitted when the service does not report them.",
    )
