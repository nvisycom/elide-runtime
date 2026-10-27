"""Schema-driven NER inference service (GLiNER2) exposed over HTTP via BentoML.

The default implementation of the NER wire contract (``elide_core.ner.v1``). A
single GLiNER2 model serves the deployment (``ELIDE_BENTO_NER_MODEL``); each request
carries a schema (entities / classifications / structures) and the service runs
one batched extraction.

Run locally::

    ELIDE_BENTO_NER_MODEL=fastino/gliner2-privacy-filter-PII-multi \\
        uv run bentoml serve elide_ner.service:NerService --reload
"""

from __future__ import annotations

import time
from collections import defaultdict

import bentoml
from bentoml.exceptions import InternalServerError, InvalidArgument
from elide_core.ner.v1 import NerRequest, NerResponse
from elide_core.runtime import get_logger, request_id

from elide_ner import backend, config
from elide_ner.image import image
from elide_ner.metrics import (
    batch_size_metric,
    entities_metric,
    labels_metric,
    rejected_metric,
    tokens_metric,
)
from elide_ner.schema import label_count, schema_key

logger = get_logger("nvisy.ner")


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
        {"name": backend.BACKEND_ENV, "value": backend.DEFAULT_BACKEND},
        {"name": config.QUANTIZE_ENV, "value": ""},
        {"name": config.COMPILE_ENV, "value": ""},
        {"name": config.MAX_TOKENS_ENV, "value": str(config.DEFAULT_MAX_TOKENS)},
        {"name": "HF_HUB_OFFLINE", "value": "1"},
        {"name": "TRANSFORMERS_OFFLINE", "value": "1"},
    ],
)
class NerService:
    def __init__(self) -> None:
        name = backend.backend_name()
        logger.info("loading NER backend %s (model=%s)", name, config.model_id())
        started = time.monotonic()
        try:
            module = backend.load()
            self.engine = module.Engine()
            self._too_long = module.TextTooLongError
            self._too_many_labels = module.TooManyLabelsError
        except Exception as exc:
            # Log before raising: the RuntimeError below is what a reader sees
            # first, and its cause is only in the chained traceback — which a
            # container log truncated to the last lines drops. `exception`
            # emits the full chain at the point of failure, so the reason is
            # in the log whether or not the chain survives.
            logger.exception(
                "failed to load NER backend %s (model=%s): %s", name, config.model_id(), exc
            )
            # Fail startup loudly rather than half-loading and 500-ing later. The
            # service fails liveness, as intended — a model that won't load (e.g.
            # not cached, with the Hub offline) should not serve traffic.
            raise RuntimeError(
                f"failed to load NER backend {name!r} model {config.model_id()!r}: {exc}"
            ) from exc
        # Elapsed, because a cold start fetches weights and the difference
        # between "slow" and "wedged" is otherwise unknowable from the log.
        logger.info(
            "%s ready in %.1fs (max_tokens=%d)",
            name,
            time.monotonic() - started,
            self.engine.max_tokens,
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
    def recognize(self, requests: list[NerRequest], ctx: bentoml.Context) -> list[NerResponse]:
        call_started = time.monotonic()
        batch_size_metric.observe(len(requests))
        rid = request_id(ctx)
        logger.info("recognize batch=%d req_id=%s", len(requests), rid)

        # Reject over-length inputs up front (the model silently truncates above
        # its token limit) — a clean 400 for the whole batch, before inference.
        # The check returns the count it measured, so usage reporting below
        # reuses this encode rather than tokenizing a second time.
        token_counts: list[int] = []
        for req in requests:
            labels = label_count(req.schema_)
            labels_metric.observe(labels)
            try:
                self.engine.check_labels(labels)
                tokens = self.engine.check_length(req.text)
            except self._too_many_labels as exc:
                rejected_metric.labels(reason="labels").inc()
                logger.warning("rejected: %s (req_id=%s)", exc, rid)
                raise InvalidArgument(str(exc)) from exc
            except self._too_long as exc:
                rejected_metric.labels(reason="tokens").inc()
                logger.warning("rejected: %s (req_id=%s)", exc, rid)
                raise InvalidArgument(str(exc)) from exc
            tokens_metric.observe(tokens)
            token_counts.append(tokens)

        # Group requests that share an identical schema + threshold so each group
        # is one batched extraction call. Distinct schemas run separately.
        groups: dict[tuple[float, str], list[int]] = defaultdict(list)
        for i, req in enumerate(requests):
            groups[schema_key(req)].append(i)

        by_index: dict[int, NerResponse] = {}
        try:
            for idxs in groups.values():
                head = requests[idxs[0]]
                responses = self.engine.recognize(
                    [requests[i].text for i in idxs],
                    head.schema_,
                    threshold=head.threshold,
                    token_counts=[token_counts[i] for i in idxs],
                )
                for i, resp in zip(idxs, responses, strict=True):
                    by_index[i] = resp
                    entities_metric.observe(len(resp.entities))
        except Exception as exc:
            # Surface inference failures as a clean 500 rather than a raw stack
            # trace; the error is visible, not silently swallowed.
            logger.exception("inference failed (req_id=%s)", rid)
            raise InternalServerError("NER inference failed") from exc

        # One line per call carrying what the metrics summarize, so a single
        # request can be explained from the log alone without a scrape: how
        # many extractions the batch became (distinct schemas do not share a
        # call), how long they took, and whether anything was found. A batch
        # that returns nothing is the symptom of a schema that does not match
        # the text, and it is otherwise indistinguishable from a quiet success.
        found = sum(len(r.entities) for r in by_index.values())
        logger.info(
            "recognized batch=%d groups=%d entities=%d elapsed=%.3fs req_id=%s",
            len(requests),
            len(groups),
            found,
            time.monotonic() - call_started,
            rid,
        )

        # Every index was covered (the groups partition range(len(requests))).
        return [by_index[i] for i in range(len(requests))]
