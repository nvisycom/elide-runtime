"""Service configuration, read from the environment.

A single GLiNER2 model serves the deployment, named by ``ELIDE_BENTO_NER_MODEL``. All
knobs are env vars so they show up in the bento manifest and can be set per
deployment without a code change.
"""

from __future__ import annotations

import os

# The single GLiNER2 model id (a Hugging Face repo id or a local path).
MODEL_ENV = "ELIDE_BENTO_NER_MODEL"
# fp16 load (gliner2 `quantize=True`) — roughly halves resident memory.
QUANTIZE_ENV = "ELIDE_BENTO_NER_QUANTIZE"
# torch.compile the model (gliner2 `compile=True`).
COMPILE_ENV = "ELIDE_BENTO_NER_COMPILE"
# Reject inputs longer than this many tokens. The model's encoder caps at 512
# and silently truncates above it, so over-length input is rejected rather than
# letting the tail (and any PII in it) be dropped unseen.
MAX_TOKENS_ENV = "ELIDE_BENTO_NER_MAX_TOKENS"
MAX_SCHEMA_BYTES_ENV = "ELIDE_BENTO_NER_MAX_SCHEMA_BYTES"

# SOTA PII GLiNER2 model (arxiv 2605.09973), Apache-2.0, CPU-viable, multilingual.
DEFAULT_MODEL = "fastino/gliner2-privacy-filter-PII-multi"
DEFAULT_MAX_TOKENS = 512
# The schema is the second dimension of an extraction's cost, and the one a
# caller could leave unbounded: tokens are capped per request and requests per
# batch, but a schema's labels and descriptions all reach the model and nothing
# limited their size. Bounded in bytes rather than labels because a count
# cannot see the text inside what it counts.
#
# 32 KiB against a measured ~1.2 KiB for the twelve-label PII schema with
# descriptions, and ~6.6 KiB for sixty-four of them: ample for any real schema,
# and far below what exhausts the memory the service declares.
DEFAULT_MAX_SCHEMA_BYTES = 32 * 1024


def model_id() -> str:
    return os.getenv(MODEL_ENV) or DEFAULT_MODEL


def quantize() -> bool:
    return _flag(QUANTIZE_ENV)


def compile_model() -> bool:
    return _flag(COMPILE_ENV)


def max_tokens() -> int:
    raw = os.getenv(MAX_TOKENS_ENV, "").strip()
    return int(raw) if raw else DEFAULT_MAX_TOKENS


def max_schema_bytes() -> int:
    raw = os.getenv(MAX_SCHEMA_BYTES_ENV, "").strip()
    return int(raw) if raw else DEFAULT_MAX_SCHEMA_BYTES


def _flag(env: str) -> bool:
    return os.getenv(env, "").strip().lower() in {"1", "true", "yes", "on"}
