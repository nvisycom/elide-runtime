"""Schema-driven NER inference service (GLiNER2) exposed over HTTP via BentoML.

The default implementation of the NER wire contract (``nvisy_core.ner.v1``). A
single GLiNER2 model serves the deployment (``NVISY_NER_MODEL``); each request
carries a schema (entities / classifications / structures) and the service runs
one batched extraction.

Run locally::

    NVISY_NER_MODEL=fastino/gliner2-privacy-filter-PII-multi \\
        uv run bentoml serve nvisy_ner.service:NerService --reload
"""

from __future__ import annotations

from collections import defaultdict

import bentoml
from bentoml.exceptions import InternalServerError, InvalidArgument
from nvisy_core.ner.v1 import NerRequest, NerResponse
from nvisy_core.runtime import get_logger, request_id
from prometheus_client import Histogram

from nvisy_ner import config
from nvisy_ner.engine import Engine, TextTooLongError

logger = get_logger("nvisy.ner")

# prometheus_client directly (bentoml.metrics is deprecated in 1.4); BentoML
# sets PROMETHEUS_MULTIPROC_DIR so this is multiprocess-safe across workers.
batch_size_metric = Histogram(
    "nvisy_ner_batch_size",
    "Number of texts merged into one recognize() call.",
    buckets=(1, 2, 4, 8, 16, 32),
)

# BentoML builds the image from this config (`bentoml build` + `containerize`);
# no hand-written Dockerfile. The requirements file is exported per-service from
# the workspace lock (scripts/gen_requirements.py); bundled source is scoped by
# bentofile.yaml's `include`. lock_python_packages=False: the file is already
# locked + hashed, so BentoML must not re-resolve it.
image = bentoml.images.Image(python_version="3.12", lock_python_packages=False).requirements_file(
    "packages/nvisy-ner/requirements.txt"
)


@bentoml.service(
    name="nvisy-inference-ner",
    image=image,
    # F32 weights are ~1.2 GB resident; give headroom for the runtime. Drop the
    # memory request when running with NVISY_NER_QUANTIZE.
    resources={"cpu": "2", "memory": "2Gi"},
    traffic={"timeout": 60},
    # Declared with defaults so they're optional + documented in the bento
    # manifest. The model is the single GLiNER2 id; HF_HUB_OFFLINE keeps the
    # service from reaching the Hub at runtime (weights are baked into the image).
    envs=[
        {"name": config.MODEL_ENV, "value": config.DEFAULT_MODEL},
        {"name": config.QUANTIZE_ENV, "value": ""},
        {"name": config.COMPILE_ENV, "value": ""},
        {"name": config.MAX_TOKENS_ENV, "value": str(config.DEFAULT_MAX_TOKENS)},
        {"name": "HF_HUB_OFFLINE", "value": "1"},
        {"name": "TRANSFORMERS_OFFLINE", "value": "1"},
    ],
)
class NerService:
    def __init__(self) -> None:
        logger.info("loading GLiNER2 (model=%s)", config.model_id())
        try:
            self.engine = Engine()
        except Exception as exc:
            # Fail startup loudly rather than half-loading and 500-ing later. The
            # service fails liveness, as intended — a model that won't load (e.g.
            # not cached, with the Hub offline) should not serve traffic.
            raise RuntimeError(f"failed to load NER model {config.model_id()!r}") from exc
        logger.info("GLiNER2 ready (max_tokens=%d)", self.engine.max_tokens)

    # Sync (not async): inference is CPU/GPU-bound and blocking. BentoML runs
    # sync endpoints in a managed thread pool, so this never blocks the event
    # loop (an async def here would, and could starve /readyz).
    #
    # batchable=True: a caller submits a list (e.g. a whole document's segments)
    # in one call, AND BentoML's dispatcher merges concurrent requests across
    # callers into a single batched extraction.
    @bentoml.api(batchable=True, max_batch_size=16, max_latency_ms=100)
    def recognize(self, requests: list[NerRequest], ctx: bentoml.Context) -> list[NerResponse]:
        batch_size_metric.observe(len(requests))
        rid = request_id(ctx)
        logger.info("recognize batch=%d req_id=%s", len(requests), rid)

        # Reject over-length inputs up front (the model silently truncates above
        # its token limit) — a clean 400 for the whole batch, before inference.
        for req in requests:
            try:
                self.engine.check_length(req.text)
            except TextTooLongError as exc:
                raise InvalidArgument(str(exc)) from exc

        # Group requests that share an identical schema + threshold so each group
        # is one batched extraction call. Distinct schemas run separately.
        groups: dict[tuple[float, str], list[int]] = defaultdict(list)
        for i, req in enumerate(requests):
            groups[_schema_key(req)].append(i)

        by_index: dict[int, NerResponse] = {}
        try:
            for idxs in groups.values():
                head = requests[idxs[0]]
                responses = self.engine.recognize(
                    [requests[i].text for i in idxs],
                    head.schema_,
                    threshold=head.threshold,
                )
                for i, resp in zip(idxs, responses, strict=True):
                    by_index[i] = resp
        except Exception as exc:
            # Surface inference failures as a clean 500 rather than a raw stack
            # trace; the error is visible, not silently swallowed.
            logger.exception("inference failed (req_id=%s)", rid)
            raise InternalServerError("NER inference failed") from exc

        # Every index was covered (the groups partition range(len(requests))).
        return [by_index[i] for i in range(len(requests))]


def _schema_key(req: NerRequest) -> tuple[float, str]:
    """A hashable key identifying requests that can share one batched call.

    Two requests batch together iff they have the same schema and threshold. The
    schema is serialized canonically (field order is stable) so equal schemas
    produce equal JSON, hence equal keys.
    """
    return (req.threshold, req.schema_.model_dump_json())
