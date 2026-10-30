"""Shared runtime helpers for the inference services.

Kept dependency-light (no BentoML import) so ``elide-core`` stays a pure
contract + utilities package: a service logger, and the request's correlation
id read from its headers.
"""

from __future__ import annotations

import logging
import os

# Header the Rust runtime sends to correlate a request across services.
REQUEST_ID_HEADER = "x-request-id"


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
