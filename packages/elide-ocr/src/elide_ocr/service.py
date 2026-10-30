"""OCR inference service (OnnxTR) exposed over HTTP via BentoML.

The default implementation of the OCR wire contract (``elide_core.ocr.v1``).
OnnxTR (docTR's models on ONNX Runtime) natively produces a ``Document -> Page
-> Block -> Line -> Word`` hierarchy with word-level geometry, which maps
directly onto our contract.

Run locally::

    uv run --project packages/elide-ocr bentoml serve elide_ocr.service:OcrService --reload
"""

from __future__ import annotations

import time

import bentoml
from bentoml.exceptions import BentoMLException, InternalServerError, InvalidArgument
from elide_core.ocr.v1 import OcrRequest, OcrResponse
from elide_core.runtime import get_logger, request_id

from elide_ocr.inference import backends as backend
from elide_ocr.inference.pipeline import OcrPipeline
from elide_ocr.inference.validation import InvalidInputError
from elide_ocr.runtime import config
from elide_ocr.runtime.image import image
from elide_ocr.runtime.metrics import batch_size_metric, words_metric

logger = get_logger("nvisy.ocr")


@bentoml.service(
    name="elide-ocr",
    image=image,
    resources={"cpu": "2"},
    traffic={"timeout": 60},
    # Declared with defaults so they're optional + documented in the bento
    # manifest. Once the init container has warmed both caches, a built-in
    # architecture resolves from ONNXTR_CACHE_DIR and a Hub model from its
    # pinned commit in the Hugging Face cache; HF_HUB_OFFLINE makes the second
    # true rather than aspirational, so request images cannot cause a request.
    envs=[
        {"name": config.BACKEND_ENV, "value": config.DEFAULT_BACKEND},
        {"name": config.DETECTION_ENV, "value": config.DEFAULT_DETECTION},
        {"name": config.RECOGNITION_ENV, "value": config.DEFAULT_RECOGNITION},
        {"name": config.DEVICE_ENV, "value": config.DEFAULT_DEVICE},
        {"name": "HF_HUB_OFFLINE", "value": "1"},
    ],
)
class OcrService:
    def __init__(self) -> None:
        settings = config.get_settings()
        self.pipeline = OcrPipeline(backend.create(settings))

    # Sync (not async): inference is CPU/GPU-bound and blocking. BentoML runs
    # sync endpoints in a managed thread pool, so this never blocks the event
    # loop (an async def here would, and could starve /readyz).
    # max_batch_size is the dispatcher's window: a longer call is split into
    # windows of this size and the results rejoined. `recognize` processes a
    # window's images one after another, so a larger window adds no
    # throughput, only waiting: the last image in it finishes after all the
    # others. 3 keeps a full window inside the request timeout: up to 15s
    # queued plus three dense pages at about 12s each is 51s, under 60.
    #
    # max_latency_ms is a deadline on queued work, and the old 120ms was far
    # shorter than one page, so concurrent callers were cancelled with `503
    # process is overloaded` rather than waited for — losing a detection over
    # a few seconds of queueing. 15s is not tuned to a measurement: it is long
    # enough that queueing never fails, and under traffic.timeout so a stuck
    # queue is cut off by the request timeout instead. See elide-ner's note.
    @bentoml.api(batchable=True, max_batch_size=3, max_latency_ms=15000)
    def recognize(
        self,
        requests: list[OcrRequest],
        ctx: bentoml.Context,
    ) -> list[OcrResponse]:
        batch_size_metric.observe(len(requests))
        rid = request_id(ctx)
        logger.info("recognize batch=%d req_id=%s", len(requests), rid)
        started = time.monotonic()
        try:
            responses = self.pipeline.recognize(requests, rid)
        except InvalidInputError as exc:
            raise InvalidArgument(str(exc)) from exc
        except BentoMLException:
            # Typed request errors (e.g. InvalidArgument for bad image bytes)
            # carry their own status; let them through unchanged.
            raise
        except Exception as exc:
            logger.exception("inference failed (req_id=%s)", rid)
            raise InternalServerError("OCR inference failed") from exc

        # One line per call carrying what the metrics summarize, so a request
        # can be explained from the log alone. Zero words across a batch means
        # a blank or unreadable scan, which is otherwise indistinguishable
        # from a quiet success.
        words = 0
        for resp in responses:
            found = sum(
                len(line.words)
                for page in resp.pages
                for block in page.blocks
                for line in block.lines
            )
            words_metric.observe(found)
            words += found
        logger.info(
            "recognized batch=%d words=%d elapsed=%.3fs req_id=%s",
            len(requests),
            words,
            time.monotonic() - started,
            rid,
        )
        return responses
