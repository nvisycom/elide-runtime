"""Transcription engine: faster-whisper, with optional pyannote diarization.

The heavy imports live inside the loaders so importing this module stays
cheap.

WhisperX is deliberately not used. It bundles the same pieces, but pins
``huggingface-hub<1.0.0``, which cannot coexist with the NER service's
``transformers>=5.15.0`` in this workspace's single lock. Assembling the
stages directly avoids that ceiling — and costs less than it sounds, since
faster-whisper reports word timings natively and needs no separate forced
alignment pass.

Two stages:

- **transcribe** (always): faster-whisper produces segments with
  float-second timings and, with ``word_timestamps``, a per-word breakdown.
- **diarize** (opt-in): pyannote assigns a speaker to each time range,
  which we intersect with the segments.

Translation happens at one boundary: faster-whisper speaks **float
seconds**, the wire contract speaks **integer milliseconds**. :func:`_to_ms`
is the only place that conversion is allowed to happen.
"""

from __future__ import annotations

import math
from typing import TYPE_CHECKING, Any

from bento_core.stt.v1 import Segment, SttResponse, Word

from bento_whisper import config

if TYPE_CHECKING:
    from collections.abc import Sequence

# faster-whisper resamples everything to 16 kHz mono on load.
SAMPLE_RATE = 16_000


class AudioTooLongError(ValueError):
    """Audio exceeds the configured duration limit."""


def _to_ms(seconds: float | None) -> int | None:
    """Float seconds to whole milliseconds, or ``None`` when absent.

    Rounds rather than truncates so a word ending at 1.9996s does not report
    1999ms while the next starts at 2000ms. Negative values (which the
    decoder can emit for a leading word) clamp to zero.
    """
    if seconds is None or (isinstance(seconds, float) and math.isnan(seconds)):
        return None
    return max(0, round(float(seconds) * 1000))


class Engine:
    """Owns the loaded models and the result translation."""

    def __init__(self) -> None:
        from bento_core.runtime import resolve_model

        self.model_id = resolve_model()
        self.device = config.device()
        self.max_duration_seconds = config.max_duration_seconds()
        self._word_timestamps = config.align()

        self._model = _load(self.model_id, self.device, config.compute_type())
        self._diarizer = _load_diarizer(self.device) if config.diarize() else None

    @property
    def diarizes(self) -> bool:
        """Whether this deployment assigns speaker labels."""
        return self._diarizer is not None

    def check_duration(self, audio: Sequence[float]) -> float:
        """Reject over-long audio, returning its duration in seconds.

        The duration is returned rather than discarded so the caller can
        report it without measuring the clip a second time.
        """
        seconds = len(audio) / SAMPLE_RATE
        if seconds > self.max_duration_seconds:
            raise AudioTooLongError(
                f"audio is {seconds:.1f}s; the limit is {self.max_duration_seconds}s"
            )
        return seconds

    def transcribe(self, audio: Any, language: str | None = None) -> SttResponse:
        """Run the pipeline over one clip and map it onto the contract."""
        segments, info = self._model.transcribe(
            audio,
            language=language,
            word_timestamps=self._word_timestamps,
        )
        # faster-whisper returns a generator; diarization needs the whole
        # clip anyway, so materialise once here rather than iterating twice.
        raw = [_segment_dict(s) for s in segments]
        detected = getattr(info, "language", None) or language

        turns = self._diarize(audio) if self._diarizer is not None else []
        return project(raw, self.model_id, detected, _to_ms(len(audio) / SAMPLE_RATE), turns)

    def _diarize(self, audio: Any) -> list[tuple[float, float, str]]:
        """Speaker turns as ``(start, end, speaker)``, in float seconds."""
        import torch

        # pyannote wants a (channel, sample) tensor plus the rate.
        waveform = torch.as_tensor(audio).reshape(1, -1)
        annotation = self._diarizer(
            {"waveform": waveform, "sample_rate": SAMPLE_RATE},
            min_speakers=config.min_speakers(),
            max_speakers=config.max_speakers(),
        )
        return [
            (turn.start, turn.end, str(speaker))
            for turn, _, speaker in annotation.itertracks(yield_label=True)
        ]


def _segment_dict(segment: Any) -> dict[str, Any]:
    """A faster-whisper segment as a plain dict.

    Decoupling the mapping from the library's named tuples keeps `project`
    testable without constructing library types.
    """
    return {
        "start": segment.start,
        "end": segment.end,
        "text": segment.text,
        "words": [
            {"start": w.start, "end": w.end, "word": w.word, "probability": w.probability}
            for w in (segment.words or [])
        ],
    }


def _load(model_id: str, device: str, compute_type: str):
    from faster_whisper import WhisperModel

    return WhisperModel(model_id, device=device, compute_type=compute_type)


def _load_diarizer(device: str):
    """The pyannote diarization pipeline.

    The model is gated on Hugging Face: without an accepted licence and a
    token the load fails, so we surface a pointed error rather than whatever
    huggingface_hub raises.
    """
    import torch
    from pyannote.audio import Pipeline

    token = config.hf_token()
    if not token:
        raise RuntimeError(
            f"{config.DIARIZE_ENV} is set but {config.HF_TOKEN_ENV} is not. "
            f"{config.diarize_model()} is gated: accept its terms on Hugging Face "
            "and supply a token."
        )
    pipeline = Pipeline.from_pretrained(config.diarize_model(), token=token)
    if pipeline is None:
        raise RuntimeError(
            f"could not load {config.diarize_model()}: the token may lack access, "
            "or its terms may not have been accepted on Hugging Face."
        )
    return pipeline.to(torch.device(device))


def project(
    segments: list[dict[str, Any]],
    model_id: str,
    language: str | None,
    duration_ms: int | None,
    turns: list[tuple[float, float, str]] | None = None,
) -> SttResponse:
    """Map transcription segments onto the typed response.

    Segments carry float-second ``start``/``end``, ``text``, and — with word
    timestamps enabled — a ``words`` list. ``turns`` are pyannote's speaker
    ranges, which are matched to segments by overlap.

    Segments with no usable timing are dropped rather than emitted with
    invented values.
    """
    turns = turns or []
    out: list[Segment] = []
    for raw in segments:
        start_ms = _to_ms(raw.get("start"))
        end_ms = _to_ms(raw.get("end"))
        if start_ms is None or end_ms is None or end_ms < start_ms:
            continue

        words = [w for w in (_word(rw) for rw in raw.get("words", [])) if w is not None]
        out.append(
            Segment(
                start_ms=start_ms,
                end_ms=end_ms,
                text=(raw.get("text") or "").strip(),
                speaker_id=_speaker_for(raw.get("start"), raw.get("end"), turns),
                language=raw.get("language") or language,
                confidence=None,
                words=words,
            )
        )

    return SttResponse(segments=out, model_id=model_id, language=language, duration_ms=duration_ms)


def _speaker_for(
    start: float | None, end: float | None, turns: list[tuple[float, float, str]]
) -> str | None:
    """The speaker whose turns overlap ``[start, end)`` the most.

    A segment can straddle a speaker change; attributing it to whoever holds
    the most of it is the same rule WhisperX applied, and keeps a segment
    from being dropped just because the boundaries disagree.
    """
    if start is None or end is None or not turns:
        return None

    totals: dict[str, float] = {}
    for turn_start, turn_end, speaker in turns:
        overlap = min(end, turn_end) - max(start, turn_start)
        if overlap > 0:
            totals[speaker] = totals.get(speaker, 0.0) + overlap
    if not totals:
        return None
    # Ties resolve to the first speaker seen, keeping the result deterministic.
    return max(totals, key=lambda s: totals[s])


def _word(raw: dict[str, Any]) -> Word | None:
    """One word, or ``None`` when it has no usable timing.

    A word without a placed span is dropped rather than emitted with a
    fabricated one.
    """
    start_ms = _to_ms(raw.get("start"))
    end_ms = _to_ms(raw.get("end"))
    text = (raw.get("word") or raw.get("text") or "").strip()
    if start_ms is None or end_ms is None or end_ms < start_ms or not text:
        return None
    return Word(start_ms=start_ms, end_ms=end_ms, text=text, confidence=_confidence(raw))


def _confidence(raw: dict[str, Any]) -> float | None:
    """A ``[0, 1]`` confidence, when the decoder reported a real one.

    faster-whisper gives each word a ``probability``. A segment-level
    ``avg_logprob`` is a log probability, not a confidence, so it is left
    absent rather than squashed into a fake score.
    """
    value = raw.get("probability")
    if value is None:
        return None
    value = float(value)
    return value if 0.0 <= value <= 1.0 else None
