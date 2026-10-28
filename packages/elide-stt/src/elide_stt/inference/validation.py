"""Decode and bound incoming audio before inference."""

import base64
import binascii
import io

from elide_stt.runtime.metrics import rejected_metric

# All backends receive mono 16 kHz audio.
SAMPLE_RATE = 16_000


class InvalidInputError(ValueError):
    """Payload cannot be decoded or exceeds an input budget."""


class AudioTooLongError(InvalidInputError):
    """Audio exceeds the configured duration limit."""


def _decode_audio(
    audio_b64: str,
    max_seconds: float | None = None,
    max_bytes: int | None = None,
):
    """Base64 audio bytes to a mono 16 kHz float32 waveform.

    Decoding goes through ffmpeg (via ``soundfile``/``librosa``) so any
    container the deployment's ffmpeg understands is accepted. A payload that
    is not decodable audio is a 400, not a 500 — it is the caller's input.

    Two independent bounds, because neither implies the other:

    - ``max_bytes`` caps the request payload, applied to the base64 text
      before decoding it, so an oversized body is refused without allocating
      the decoded bytes.
    - ``max_seconds`` caps the decode itself, so a long input costs one
      bounded read rather than a full decode and resample of however much
      audio the caller sent.

    A duration limit cannot be turned into a byte limit: how many bytes a
    given duration occupies depends on the source's sample rate, channel
    count and codec, none of which are known until the container is opened.
    Deriving one from the other would reject valid audio - a 48 kHz stereo
    WAV is six times the size of the 16 kHz mono waveform it decodes to.
    """
    if max_bytes is not None:
        # Applied to the base64 text: 4 characters encode 3 bytes, so this
        # refuses an oversized body before allocating the decoded form.
        encoded_limit = (max_bytes + 2) // 3 * 4
        if len(audio_b64) > encoded_limit:
            rejected_metric.labels(reason="oversized").inc()
            raise InvalidInputError(
                f"audio payload is {len(audio_b64)} base64 characters; "
                f"the limit is {encoded_limit} ({max_bytes} bytes)"
            )
    try:
        raw = base64.b64decode(audio_b64, validate=True)
    except (binascii.Error, ValueError) as exc:
        rejected_metric.labels(reason="base64").inc()
        raise InvalidInputError("audio is not valid base64") from exc
    if not raw:
        rejected_metric.labels(reason="empty").inc()
        raise InvalidInputError("audio is empty")

    import librosa

    try:
        waveform, _ = librosa.load(io.BytesIO(raw), sr=SAMPLE_RATE, mono=True, duration=max_seconds)
    except Exception as exc:
        raise InvalidInputError("audio bytes are not decodable audio") from exc
    if waveform.size == 0:
        raise InvalidInputError("audio decodes to an empty waveform")
    return waveform
