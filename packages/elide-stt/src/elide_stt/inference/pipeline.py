"""Prepare bounded audio and transcribe independently of HTTP handling."""

from elide_core.stt.v1 import SttRequest, SttResponse

from elide_stt.inference.backends.base import SttBackend
from elide_stt.inference.validation import AudioTooLongError, _decode_audio
from elide_stt.runtime.config import SttSettings
from elide_stt.runtime.metrics import duration_metric, rejected_metric


class SttPipeline:
    def __init__(self, backend: SttBackend, settings: SttSettings) -> None:
        self.backend = backend
        self.settings = settings

    def transcribe(self, request: SttRequest) -> SttResponse:
        audio = _decode_audio(
            request.audio,
            max_seconds=self.backend.max_duration_seconds + 1,
            max_bytes=self.settings.max_bytes,
        )
        try:
            seconds = self.backend.check_duration(audio)
        except AudioTooLongError:
            rejected_metric.labels(reason="too_long").inc()
            raise
        duration_metric.observe(seconds)
        return self.backend.transcribe(audio, language=request.language)
