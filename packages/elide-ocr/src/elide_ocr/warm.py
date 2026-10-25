"""Download the docTR model pair into docTR's cache, then exit.

Run by this service's one-shot init container before the service starts::

    python -m elide_ocr.warm

docTR fetches from ``doctr-static.mindee.com`` rather than the Hugging Face
Hub, and instantiating the predictor is its only public path that populates
the cache — so this cannot share the Hub-based warm the other services use.
Shared pieces come from ``elide_core.warm``.
"""

from __future__ import annotations

import sys

from elide_core.warm import hand_over, log, require_env


def main() -> None:
    from doctr.models import ocr_predictor

    cache = require_env("DOCTR_CACHE_DIR")
    pair = require_env("ELIDE_BENTO_MODEL_NAME")
    det, _, rec = pair.partition("+")
    if not rec:
        sys.exit(f"ELIDE_BENTO_MODEL_NAME must be 'det+rec', got {pair!r}.")
    log(f"warming {pair}")
    ocr_predictor(det, rec, pretrained=True)
    hand_over(cache)
    log("warm")


if __name__ == "__main__":
    main()
