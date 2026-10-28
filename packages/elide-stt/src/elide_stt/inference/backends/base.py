"""Contract implemented by transcription backends."""

from collections.abc import Sequence
from typing import Any, Protocol

from elide_core.stt.v1 import SttResponse


class SttBackend(Protocol):
    model_id: str
    device: str
    max_duration_seconds: int

    @property
    def diarizes(self) -> bool: ...

    def check_duration(self, audio: Sequence[float]) -> float: ...

    def transcribe(self, audio: Any, language: str | None = None) -> SttResponse: ...
