"""Download the docTR model pair into docTR's cache, then exit.

Run by this service's one-shot init container before the service starts::

    python -m elide_ocr.warm

docTR fetches from ``doctr-static.mindee.com`` rather than the Hugging Face
Hub, and instantiating the predictor is its only public path that populates
the cache — so this cannot share the Hub-based warm the other services use.
Shared pieces come from ``elide_core.warm``.
"""

from __future__ import annotations

import logging
import os
import sys

from elide_core.warm import hand_over, require_env

from elide_ocr.runtime import config

logger = logging.getLogger("nvisy.ocr.warm")


def main() -> None:
    from doctr.models import ocr_predictor

    logging.basicConfig(
        level=os.getenv("LOG_LEVEL", "INFO").upper(),
        format="%(asctime)s %(levelname)s %(name)s: %(message)s",
    )
    cache = require_env("DOCTR_CACHE_DIR")
    pair = config.OcrSettings().model_name
    det, _, rec = pair.partition("+")
    if not rec:
        sys.exit(f"ELIDE_BENTO_MODEL_NAME must be 'det+rec', got {pair!r}.")
    logger.info("warming %s", pair)
    ocr_predictor(det, rec, pretrained=True)
    hand_over(cache)
    logger.info("warm")


if __name__ == "__main__":
    main()
