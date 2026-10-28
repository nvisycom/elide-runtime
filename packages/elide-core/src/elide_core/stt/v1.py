"""STT wire contract, version 1.

Request carries base64-encoded audio bytes plus optional filename and language
hint; the response is a flat list of :class:`Segment`, each with millisecond
timings, text, and optional diarization speaker label, language, confidence,
and per-word breakdown.

These Python models own the HTTP contract and generate the Rust wire types.
Field names use snake_case on the wire and in both language implementations.

**Timings are integer milliseconds**, not float seconds. Most engines (Whisper,
WhisperX, and every hosted API surveyed bar one) emit float seconds, so the
service converts at its boundary rather than leaking a second unit onto the
wire.
"""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict, Field, RootModel, model_validator

from elide_core.types import Probability


class _Model(BaseModel):
    """Snake_case wire models, with model metadata field names allowed."""

    model_config = ConfigDict(protected_namespaces=())


class SttRequest(_Model):
    """One transcription request: audio bytes plus advisory hints."""

    audio: str = Field(description="Base64-encoded audio bytes (WAV, MP3, FLAC, ...).")
    filename: str | None = Field(
        default=None,
        description="Original filename, when known. Used for container/codec detection.",
    )
    language: str | None = Field(
        default=None,
        description=(
            "Caller-asserted language as a BCP-47 tag. Omitted means auto-detect. "
            "An asserted language is a hint the engine may use to pick a variant."
        ),
    )


class Word(_Model):
    """One recognized word with its own timing."""

    start_ms: int = Field(ge=0, description="Word start, milliseconds from clip start.")
    end_ms: int = Field(ge=0, description="Word end, milliseconds from clip start.")
    text: str = Field(description="The word as recognized.")
    confidence: Probability | None = None

    @model_validator(mode="after")
    def _check_span(self) -> Word:
        if self.end_ms < self.start_ms:
            raise ValueError("end_ms must be greater than or equal to start_ms")
        return self


class Segment(_Model):
    """A contiguous stretch of speech, optionally attributed to a speaker.

    ``speaker_id`` is populated only by a diarizing deployment; it is the
    engine's own label (``"SPEAKER_00"``, ...), not a stable identity across
    calls. Consumers treat it as an opaque grouping key within one transcript.
    """

    start_ms: int = Field(ge=0, description="Segment start, milliseconds from clip start.")
    end_ms: int = Field(ge=0, description="Segment end, milliseconds from clip start.")
    text: str = Field(description="The segment transcript.")
    speaker_id: str | None = Field(
        default=None,
        description="Diarization speaker label, when the deployment diarizes.",
    )
    language: str | None = Field(
        default=None, description="Detected language for this segment, as a BCP-47 tag."
    )
    confidence: Probability | None = None
    words: list[Word] = Field(
        default_factory=list,
        description="Per-word breakdown, when the engine reports word timings.",
    )

    @model_validator(mode="after")
    def _check_span(self) -> Segment:
        if self.end_ms < self.start_ms:
            raise ValueError("end_ms must be greater than or equal to start_ms")
        return self


class SttResponse(_Model):
    """One per-call transcription response."""

    segments: list[Segment] = Field(default_factory=list)
    model_id: str = Field(description="Identifier of the model that produced this.")
    language: str | None = Field(
        default=None,
        description="Overall detected language for the clip, as a BCP-47 tag.",
    )
    duration_ms: int | None = Field(
        default=None, ge=0, description="Audio duration in milliseconds, when known."
    )


class SttInput(_Model):
    """BentoML's named request envelope for one transcription."""

    request: SttRequest


class SttOutput(RootModel[SttResponse]):
    """The transcription response object, without an additional JSON wrapper."""
