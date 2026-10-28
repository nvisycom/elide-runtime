"""Schema-driven NER inference service (GLiNER2) exposed over HTTP via BentoML.

The default implementation of the NER wire contract (``elide_core.ner.v1``). A
single GLiNER2 model serves the deployment (``ELIDE_BENTO_NER_MODEL``); each request
carries a schema of entity labels and the service runs
one batched extraction.

Run locally::

    ELIDE_BENTO_NER_MODEL=fastino/gliner2-privacy-filter-PII-multi \\
        uv run bentoml serve elide_ner.service:NerService --reload
"""

from __future__ import annotations

import time

import bentoml
from bentoml.exceptions import InternalServerError, InvalidArgument
from elide_core.ner.v1 import NerRequest, NerResponse, Schema
from elide_core.runtime import get_logger, request_id
from pydantic import field_validator

from elide_ner.inference import backends as backend
from elide_ner.inference.pipeline import NerPipeline
from elide_ner.inference.validation import (
    SchemaTooLargeError,
    TextTooLongError,
    check_schema_size,
    schema_size,
)
from elide_ner.runtime import config
from elide_ner.runtime.image import image
from elide_ner.runtime.metrics import batch_size_metric, rejected_metric

logger = get_logger("nvisy.ner")


class ValidatedNerRequest(NerRequest):
    """Enforce the schema budget before the adaptive dispatcher sees a caller."""

    @field_validator("schema_")
    @classmethod
    def check_schema_budget(cls, schema: Schema) -> Schema:
        size = schema_size(schema)
        limit = config.get_settings().max_schema_bytes
        try:
            check_schema_size(size, limit)
        except SchemaTooLargeError as exc:
            rejected_metric.labels(reason="schema").inc()
            logger.warning("rejected: %s", exc)
            raise
        return schema


@bentoml.service(
    name="elide-ner",
    image=image,
    # Sized to the batch this service accepts, which is what 2Gi did not do:
    # the weights alone are ~1.92 GiB resident, and an extraction adds a few
    # hundred MiB of activations on top, so a batch past about four exhausted
    # the limit and the worker was killed mid-request. Measured peak at a batch
    # of 16 with a twelve-label schema is ~2.35 GiB; 4Gi is a deliberate margin
    # over that for longer segments and larger schemas.
    #
    # max_batch_size below and this figure are one decision, not two: raising
    # either without the other is what broke. Drop this when running with
    # ELIDE_BENTO_NER_QUANTIZE, whose weights are far smaller.
    resources={"cpu": "2", "memory": "4Gi"},
    traffic={"timeout": 60},
    # Declared with defaults so they're optional + documented in the bento
    # manifest. The model is the single GLiNER2 id; HF_HUB_OFFLINE keeps the
    # service from reaching the Hub at runtime (weights are baked into the image).
    envs=[
        {"name": config.MODEL_ENV, "value": config.DEFAULT_MODEL},
        {"name": config.BACKEND_ENV, "value": config.DEFAULT_BACKEND},
        {"name": config.QUANTIZE_ENV, "value": ""},
        {"name": config.COMPILE_ENV, "value": ""},
        {"name": config.MAX_TOKENS_ENV, "value": str(config.DEFAULT_MAX_TOKENS)},
        {"name": config.MAX_SCHEMA_BYTES_ENV, "value": str(config.DEFAULT_MAX_SCHEMA_BYTES)},
        {"name": "HF_HUB_OFFLINE", "value": "1"},
        {"name": "TRANSFORMERS_OFFLINE", "value": "1"},
    ],
)
class NerService:
    def __init__(self) -> None:
        settings = config.get_settings()
        name = settings.backend
        logger.info("loading NER backend %s (model=%s)", name, settings.model_id)
        started = time.monotonic()
        try:
            self.pipeline = NerPipeline(backend.create(settings))
        except Exception as exc:
            # Log before raising: the RuntimeError below is what a reader sees
            # first, and its cause is only in the chained traceback — which a
            # container log truncated to the last lines drops. `exception`
            # emits the full chain at the point of failure, so the reason is
            # in the log whether or not the chain survives.
            logger.exception(
                "failed to load NER backend %s (model=%s): %s", name, settings.model_id, exc
            )
            # Fail startup loudly rather than half-loading and 500-ing later. The
            # service fails liveness, as intended — a model that won't load (e.g.
            # not cached, with the Hub offline) should not serve traffic.
            raise RuntimeError(
                f"failed to load NER backend {name!r} model {settings.model_id!r}: {exc}"
            ) from exc
        # Elapsed, because a cold start fetches weights and the difference
        # between "slow" and "wedged" is otherwise unknowable from the log.
        logger.info(
            "%s ready in %.1fs (max_tokens=%d)",
            name,
            time.monotonic() - started,
            self.pipeline.backend.max_tokens,
        )

    # Sync (not async): inference is CPU/GPU-bound and blocking. BentoML runs
    # sync endpoints in a managed thread pool, so this never blocks the event
    # loop (an async def here would, and could starve /readyz).
    #
    # batchable=True: a caller submits a list (e.g. a whole document's segments)
    # in one call, AND BentoML's dispatcher merges concurrent requests across
    # callers into a single batched extraction.
    #
    # max_batch_size is the dispatcher's window: a longer call is split into
    # windows of this size and the results rejoined, so it bounds how much
    # work one extraction does, not how much a caller may send.
    #
    # max_latency_ms is a deadline, and the one that matters here. The
    # dispatcher cancels queued work whose projected latency exceeds it, and a
    # cancelled request surfaces as `503 process is overloaded`. Set shorter
    # than a call takes, it throws work away rather than waiting for it —
    # which loses a detection over a few seconds of queueing, the worst
    # trade available. The old 100ms was far shorter than one extraction.
    #
    # 15s is not tuned to any measurement, and should not be: load, hardware
    # and model all move it. It is simply long enough that queueing is never
    # what fails, and short enough to stay under traffic.timeout below, so a
    # genuinely stuck queue is cut off by the request timeout instead — one
    # deadline doing the work, not two disagreeing.
    #
    # A caller with nothing to coalesce still dispatches immediately, so
    # raising this costs idle latency nothing.
    @bentoml.api(batchable=True, max_batch_size=32, max_latency_ms=15000)
    def recognize(
        self, requests: list[ValidatedNerRequest], ctx: bentoml.Context
    ) -> list[NerResponse]:
        call_started = time.monotonic()
        batch_size_metric.observe(len(requests))
        rid = request_id(ctx)
        logger.info("recognize batch=%d req_id=%s", len(requests), rid)

        try:
            responses, groups = self.pipeline.recognize(requests)
        except (SchemaTooLargeError, TextTooLongError) as exc:
            logger.warning("rejected: %s (req_id=%s)", exc, rid)
            raise InvalidArgument(str(exc)) from exc
        except Exception as exc:
            logger.exception("inference failed (req_id=%s)", rid)
            raise InternalServerError("NER inference failed") from exc

        # One line per call carrying what the metrics summarize, so a single
        # request can be explained from the log alone without a scrape: how
        # many extractions the batch became (distinct schemas do not share a
        # call), how long they took, and whether anything was found. A batch
        # that returns nothing is the symptom of a schema that does not match
        # the text, and it is otherwise indistinguishable from a quiet success.
        found = sum(len(r.entities) for r in responses)
        logger.info(
            "recognized batch=%d groups=%d entities=%d elapsed=%.3fs req_id=%s",
            len(requests),
            groups,
            found,
            time.monotonic() - call_started,
            rid,
        )

        return responses
