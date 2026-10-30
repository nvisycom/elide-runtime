"""Download the OCR model pair into its caches, then exit.

Run by this service's one-shot init container before the service starts::

    python -m elide_ocr.warm

Builds the service's own engine: constructing the predictor is OnnxTR's only
public path that populates its cache, and building the same engine the service
builds means the warmer fetches exactly what the service will load, with the
same validation. A built-in architecture lands in ``ONNXTR_CACHE_DIR``, a Hub
model in the Hugging Face cache under ``HF_HOME``; both are required, because
which one a model uses is a setting. The image declares ``HF_HUB_OFFLINE=1``
for the service, so the init container turns it off for itself. Shared pieces
come from ``elide_core.warm``.
"""

from __future__ import annotations

import logging
from pathlib import Path

from elide_core.warm import configure_logging, hand_over, require_env

from elide_ocr.inference import backends
from elide_ocr.runtime import config

logger = logging.getLogger("nvisy.ocr.warm")


def main() -> None:
    configure_logging()
    caches = [require_env("ONNXTR_CACHE_DIR"), require_env("HF_HOME")]
    settings = config.OcrSettings()
    logger.info("warming %s", settings.model_id)
    backends.create(settings)
    for cache in caches:
        # A pair of built-ins never touches the Hub cache, which would then
        # not exist to hand over.
        Path(cache).mkdir(parents=True, exist_ok=True)
        hand_over(cache)
    logger.info("warm")


if __name__ == "__main__":
    main()
