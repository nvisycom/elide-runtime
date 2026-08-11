"""Shared runtime helpers for the inference services.

Kept dependency-light (no BentoML import) so ``nvisy-core`` stays a pure
contract + utilities package: model-path resolution for bring-your-own-weights,
and a logger that stamps every line with the request's correlation id.
"""

from __future__ import annotations

import logging
import os
from pathlib import Path

# Filesystem path to weights; defaults to the conventional /models mount.
MODEL_PATH_ENV = "NVISY_MODEL_PATH"
# Model identifier (Hugging Face repo id, or a framework name like an OCR
# version) loaded when no weights are present at the path above.
MODEL_NAME_ENV = "NVISY_MODEL_NAME"
# Header the Rust runtime sends to correlate a request across services.
REQUEST_ID_HEADER = "x-request-id"


def resolve_model() -> str:
    """Resolve which model a service should load (bring-your-own-weights).

    If ``$NVISY_MODEL_PATH`` points at a directory that contains weights, load
    those (mounted/BYO weights); otherwise load ``$NVISY_MODEL_NAME``. Both envs
    are declared on the service with default ``value``s (``/models`` and the
    service's default model id), so BentoML always injects them — the path
    defaulting to an empty ``/models`` mount means "no BYO weights, use the
    named model". Returns a local path or a model identifier; both PaddleOCR and
    ``GLiNER.from_pretrained`` accept either form.
    """
    path = os.getenv(MODEL_PATH_ENV)
    if path:
        weights = Path(path)
        if weights.is_dir() and any(weights.iterdir()):
            return path
    name = os.getenv(MODEL_NAME_ENV)
    if not name:
        # Served bentos inject NVISY_MODEL_NAME; guard direct/test invocation.
        raise RuntimeError(f"{MODEL_NAME_ENV} is not set and no weights at {MODEL_PATH_ENV}")
    return name


def get_logger(name: str) -> logging.Logger:
    """A logger for a service; honors the LOG_LEVEL env var (default INFO).

    No handler is attached: records propagate to BentoML's configured root
    handler, so each line is emitted once (in BentoML's format, with its
    trace/span context). Attaching our own handler would double every line.
    """
    logger = logging.getLogger(name)
    logger.setLevel(os.getenv("LOG_LEVEL", "INFO").upper())
    return logger


def request_id(ctx: object) -> str | None:
    """Pull the correlation id from a BentoML request context, or ``None``.

    BentoML exposes request headers on ``ctx.request.headers``. We read the
    runtime-supplied ``x-request-id`` so service logs can be correlated with the
    caller; absent it (or no headers at all), returns ``None`` rather than
    raising or inventing a sentinel.
    """
    headers = getattr(getattr(ctx, "request", None), "headers", None)
    if headers is None:
        return None
    try:
        return headers.get(REQUEST_ID_HEADER)
    except (AttributeError, TypeError):
        return None
