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

import os
import time
from collections import defaultdict

import bentoml
from bentoml.exceptions import InternalServerError, InvalidArgument
from elide_core.ner.v1 import NerRequest, NerResponse
from elide_core.runtime import get_logger, request_id
from prometheus_client import Histogram

from elide_ner import backend, config

logger = get_logger("nvisy.ner")

# prometheus_client directly (bentoml.metrics is deprecated in 1.4); BentoML
# sets PROMETHEUS_MULTIPROC_DIR so this is multiprocess-safe across workers.
batch_size_metric = Histogram(
    "elide_ner_batch_size",
    "Number of texts merged into one recognize() call.",
    buckets=(1, 2, 4, 8, 16, 32),
)

# BentoML builds the image from this config (`bentoml build` + `containerize`);
# no hand-written Dockerfile. The requirements file is exported per-service from
# the workspace lock by `make services` (scripts/gen_requirements.py), not
# committed; bundled source is scoped by
# bentofile.yaml's `include`. lock_python_packages=False: the file is already
# locked + hashed, so BentoML must not re-resolve it.
# Build-time image spec, skipped inside the container. `requirements_file`
# reads the file when this module is imported, and the path it needs differs
# between the two contexts (`packages/...` at build time, `src/packages/...`
# under the bento root), so evaluating it at runtime raises FileNotFoundError
# and the service never starts. BENTOML_CONTAINERIZED is set in the image and
# unset during a build, which is the distinction.
#
# build_include copies the bundled workspace source to ./src *before* the
# image's `uv pip install`, which is what makes the editable path dependencies
# in requirements.txt resolvable: the install then runs with
# INSTALL_ROOT=./src, where `./packages/elide-*` are real directories.
#
# lock_python_packages=False: the file is already locked and hashed by
# scripts/gen_requirements.py, so BentoML must not re-resolve it.
image = (
    None
    if os.getenv("BENTOML_CONTAINERIZED")
    else (
        bentoml.images.Image(python_version="3.12", lock_python_packages=False)
        # The torch pins are `+cpu` builds from the PyTorch CPU index, which
        # requirements.txt carries as an --extra-index-url. That index also mirrors
        # unrelated packages (certifi, filelock, ...) at versions that may not match
        # ours, and uv defaults to `first-index`: it refuses to look past the first
        # index that carries a name at all, so the install fails on those.
        #
        # `--index-strategy` is not a directive a requirements file accepts, and a
        # RUN cannot export an env var to the next layer, so the setting goes in
        # uv's system config, which uv reads with no further configuration. This
        # runs before the install step.
        #
        # "unsafe" describes resolution, not installation: every pin in
        # requirements.txt is hash-locked, so an artifact from any index is
        # rejected unless its SHA-256 matches the lockfile. The hashes, not the
        # index order, are what make this safe.
        .run(
            "mkdir -p /etc/uv && printf 'index-strategy = \"unsafe-best-match\"\\n'"
            " > /etc/uv/uv.toml"
        )
        .build_include("packages", "pyproject.toml", "uv.lock")
        .requirements_file("packages/elide-ner/requirements.txt")
    )
)


@bentoml.service(
    name="elide-ner",
    image=image,
    # F32 weights are ~1.2 GB resident; give headroom for the runtime. Drop the
    # memory request when running with ELIDE_BENTO_NER_QUANTIZE.
    resources={"cpu": "2", "memory": "2Gi"},
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
    # max_batch_size is a hard ceiling, not a hint: the dispatcher rejects a
    # single call larger than it with `503 process is overloaded` instead of
    # splitting it, measured at 0.1s for n=33 against 6.3s for n=32. Callers
    # chunk to this number (elide-bentoml reads it from BATCH_LIMIT), so the
    # ceiling exists for a caller that does not.
    #
    # 32 rather than 16: sentences are cheap, and doubling cost ~7 MiB of the
    # 2Gi request (1.86 -> 1.867 GiB resident, nearly all of it model weights).
    # It is bounded by traffic.timeout instead — 32 takes ~6.3s, so 64 would
    # sit near 13s and leave little margin under the 60s limit.
    #
    # max_latency_ms is a queue deadline, not a coalescing window: the
    # dispatcher cancels a request whose projected latency exceeds it, and a
    # cancelled request surfaces as `503 process is overloaded`. At the old
    # 100ms a single call takes ~300ms, so the 4th concurrent request was
    # certain to miss the deadline and be dropped before running — measured as
    # 503 for n=4 and for 6 of 8 at n=8, with nothing overloaded.
    #
    # 15s lets them queue instead: n=8 all succeed, and single-request latency
    # is unchanged (p50 0.31s, p95 0.50s at both 100ms and 15s over 20 calls),
    # because a request with nothing to coalesce dispatches immediately either
    # way. Load now shows up as waiting rather than errors, which is what a
    # caller can actually respond to. Under traffic.timeout so a genuinely
    # stuck queue still fails before the HTTP deadline.
    @bentoml.api(batchable=True, max_batch_size=32, max_latency_ms=15000)
    def recognize(self, requests: list[NerRequest], ctx: bentoml.Context) -> list[NerResponse]:
        batch_size_metric.observe(len(requests))
        rid = request_id(ctx)
        logger.info("recognize batch=%d req_id=%s", len(requests), rid)

        # Reject over-length inputs up front (the model silently truncates above
        # its token limit) — a clean 400 for the whole batch, before inference.
        # The check returns the count it measured, so usage reporting below
        # reuses this encode rather than tokenizing a second time.
        token_counts: list[int] = []
        for req in requests:
            try:
                token_counts.append(self.engine.check_length(req.text))
            except self._too_long as exc:
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
                    token_counts=[token_counts[i] for i in idxs],
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
