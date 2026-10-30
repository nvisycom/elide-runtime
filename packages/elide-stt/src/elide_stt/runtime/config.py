"""Service configuration, read from the environment.

A faster-whisper model named by ``ELIDE_BENTO_STT_MODEL`` serves the
deployment, with optional pyannote diarization for speaker labels. All knobs
are env vars so they show up in the bento manifest and can be set per
deployment without a code change.
"""

from __future__ import annotations

from functools import lru_cache
from pathlib import Path
from typing import Literal

from pydantic import Field, SecretStr, computed_field, field_validator, model_validator
from pydantic_settings import BaseSettings, SettingsConfigDict

# The transcription model: a faster-whisper size ("large-v3-turbo"), a
# CTranslate2 repo id on the Hub, or a local directory holding converted
# weights. faster-whisper itself tells them apart, a directory first.
MODEL_ENV = "ELIDE_BENTO_STT_MODEL"
# Compute type for faster-whisper ("int8" on CPU, "float16" on GPU).
COMPUTE_TYPE_ENV = "ELIDE_BENTO_STT_COMPUTE_TYPE"
# Device to run on ("cpu" or "cuda").
DEVICE_ENV = "ELIDE_BENTO_STT_DEVICE"
# Enable pyannote diarization (speaker labels). Off by default: it loads a
# second model, needs a gated HF token, and roughly doubles latency.
DIARIZE_ENV = "ELIDE_BENTO_STT_DIARIZE"
# HF token for the gated pyannote pipeline. Required only when diarizing.
HF_TOKEN_ENV = "ELIDE_BENTO_STT_HF_TOKEN"
# Diarization pipeline id.
DIARIZE_MODEL_ENV = "ELIDE_BENTO_STT_DIARIZE_MODEL"
# Emit per-word timings. On by default: the contract carries them, and
# faster-whisper produces them natively (no separate alignment pass).
ALIGN_ENV = "ELIDE_BENTO_STT_WORD_TIMESTAMPS"
# Reject audio longer than this. Transcription is O(duration) and a long clip
# can occupy a worker for minutes.
MAX_DURATION_ENV = "ELIDE_BENTO_STT_MAX_DURATION_SECONDS"
# Reject request payloads larger than this, before decoding. Independent of
# the duration limit: how many bytes a given duration occupies depends on the
# source's rate, channel count and codec, none of which are known until the
# container is opened.
MAX_BYTES_ENV = "ELIDE_BENTO_STT_MAX_BYTES"
# Bound the diarization search when the caller gives no hint.
MIN_SPEAKERS_ENV = "ELIDE_BENTO_STT_MIN_SPEAKERS"
MAX_SPEAKERS_ENV = "ELIDE_BENTO_STT_MAX_SPEAKERS"

# faster-whisper large-v3-turbo: MIT, 99 languages, native word timestamps,
# 2-5x faster than large-v3 for a small accuracy cost (a pruned 4-layer
# decoder). Declared as the ELIDE_BENTO_STT_MODEL default in the service (single
# source of truth).
#
# Not the WER leader on the Open ASR Leaderboard - roughly ten open models
# score better - but the leaders (Parakeet, Canary, Voxtral) each need NeMo or
# vLLM, are GPU-bound, and several are English-only. This picks ecosystem fit
# and CPU viability over raw accuracy; see the README.
DEFAULT_MODEL = "large-v3-turbo"

# CC-BY-4.0 (gated on Hugging Face). Supersedes speaker-diarization-3.1 and
# is WhisperX's current default reference.
DEFAULT_DIARIZE_MODEL = "pyannote/speaker-diarization-community-1"
DEFAULT_MAX_DURATION_SECONDS = 3600
# 512 MiB: comfortably above an hour of high-rate stereo PCM, while still
# bounding what one request can make a worker read.
DEFAULT_MAX_BYTES = 512 * 1024 * 1024


BACKEND_ENV = "ELIDE_BENTO_STT_BACKEND"
DEFAULT_BACKEND = "faster_whisper"


class SttSettings(BaseSettings):
    """Immutable startup configuration."""

    model_config = SettingsConfigDict(
        env_prefix="ELIDE_BENTO_STT_", env_ignore_empty=True, frozen=True, populate_by_name=True
    )

    model_id: str = Field(DEFAULT_MODEL, validation_alias=MODEL_ENV, min_length=1)
    backend: str = Field(DEFAULT_BACKEND, min_length=1)
    device: Literal["cpu", "cuda"] = "cpu"
    compute_type: str | None = None
    diarize: bool = False
    hf_token: SecretStr | None = None
    diarize_model: str = DEFAULT_DIARIZE_MODEL
    word_timestamps: bool = True
    max_duration_seconds: int = Field(DEFAULT_MAX_DURATION_SECONDS, gt=0)
    max_bytes: int = Field(DEFAULT_MAX_BYTES, gt=0)
    min_speakers: int | None = Field(None, gt=0)
    max_speakers: int | None = Field(None, gt=0)

    @field_validator("model_id")
    @classmethod
    def check_local_model(cls, value: str) -> str:
        # A directory is loaded where it lies, so the warmer fetches nothing
        # for it; without CTranslate2's weights the service would then fail
        # at load, after the warmer reported success. Refused here instead,
        # where both read it. Not a fallback to the default model: a named
        # directory is the operator's choice, and silently serving other
        # weights would hide the mistake.
        path = Path(value)
        if path.is_dir() and not (path / "model.bin").is_file():
            raise ValueError(
                f"{value} is a directory without model.bin; it must hold a "
                "CTranslate2 conversion of a Whisper model"
            )
        return value

    @computed_field
    @property
    def resolved_compute_type(self) -> str:
        return self.compute_type or ("float16" if self.device == "cuda" else "int8")

    @model_validator(mode="after")
    def check_speaker_range(self) -> SttSettings:
        if (
            self.min_speakers is not None
            and self.max_speakers is not None
            and self.min_speakers > self.max_speakers
        ):
            raise ValueError("min_speakers must not exceed max_speakers")
        return self


@lru_cache(maxsize=1)
def get_settings() -> SttSettings:
    """Read configuration once per process, outside package import."""
    return SttSettings()
