"""Speech-to-text inference service (faster-whisper) exposed over HTTP via BentoML.

The default implementation of the STT wire contract
(``elide_core.stt.v1``), and the service the Rust client's ``BentoStt``
backend has always been written against.

Diarization is opt-in (``ELIDE_BENTO_STT_DIARIZE``): it loads a second, gated
model and roughly doubles latency, so a deployment that does not need speaker
labels should not pay for them.

Run locally::

    ELIDE_BENTO_STT_MODEL=large-v3-turbo \\
        uv run --project packages/elide-stt bentoml serve elide_stt.service:SttService --reload
"""

from __future__ import annotations

import time

import bentoml
from bentoml.exceptions import BentoMLException, InternalServerError, InvalidArgument
from elide_core.runtime import get_logger, request_id
from elide_core.stt.v1 import SttRequest, SttResponse

from elide_stt.inference import backends as backend
from elide_stt.inference.pipeline import SttPipeline
from elide_stt.inference.validation import InvalidInputError
from elide_stt.runtime import config
from elide_stt.runtime.image import image

logger = get_logger("nvisy.stt")


@bentoml.service(
    name="elide-stt",
    image=image,
    resources={"cpu": "4"},
    # Transcription is O(audio duration); a long clip legitimately takes
    # minutes, so the ceiling is well above the other services'.
    traffic={"timeout": 900},
    # HF_HUB_OFFLINE: once stt-init has warmed the cache, the models resolve
    # from disk; without it, loading a model by name still asks the Hub for
    # its latest revision on every start.
    envs=[
        {"name": config.BACKEND_ENV, "value": config.DEFAULT_BACKEND},
        {"name": config.MODEL_ENV, "value": config.DEFAULT_MODEL},
        {"name": "HF_HUB_OFFLINE", "value": "1"},
    ],
)
class SttService:
    def __init__(self) -> None:
        self.settings = config.get_settings()
        logger.info("loading %s (model=%s)", self.settings.backend, self.settings.model_id)
        started = time.monotonic()
        try:
            self.pipeline = SttPipeline(backend.create(self.settings), self.settings)
        except Exception as exc:
            logger.exception("failed to load faster-whisper: %s", exc)
            raise RuntimeError(f"failed to load faster-whisper: {exc}") from exc
        logger.info(
            "engine ready in %.1fs (model=%s device=%s diarize=%s)",
            time.monotonic() - started,
            self.pipeline.backend.model_id,
            self.pipeline.backend.device,
            self.pipeline.backend.diarizes,
        )

    # Not batchable: clips vary from seconds to an hour, so batching would let
    # one long clip hold short ones hostage behind it. Sync (not async):
    # inference is CPU/GPU-bound and blocking, and BentoML runs sync endpoints
    # in a managed thread pool, so this never blocks the event loop.
    @bentoml.api
    def transcribe(self, request: SttRequest, ctx: bentoml.Context) -> SttResponse:
        rid = request_id(ctx)
        started = time.monotonic()
        logger.info("transcribe req_id=%s", rid)
        try:
            response = self.pipeline.transcribe(request)
        except InvalidInputError as exc:
            logger.warning("rejected: %s (req_id=%s)", exc, rid)
            raise InvalidArgument(str(exc)) from exc
        except BentoMLException:
            # Typed request errors carry their own status; let them through.
            raise
        except Exception as exc:
            logger.exception("inference failed (req_id=%s)", rid)
            raise InternalServerError("STT inference failed") from exc
        logger.info("transcribed elapsed=%.3fs req_id=%s", time.monotonic() - started, rid)
        return response
