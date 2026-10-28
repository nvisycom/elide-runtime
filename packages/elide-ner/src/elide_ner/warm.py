"""Download the NER model into the Hugging Face cache, then exit.

Run by this service's one-shot init container before the service starts::

    python -m elide_ner.warm

Lives with the service rather than in ``elide-core`` because it imports
``huggingface_hub``, which only this service and ``elide-stt`` depend on.
Shared pieces — the env helper and ownership handover — come from
``elide_core.warm``.
"""

from __future__ import annotations

import logging
import os

from elide_core.warm import hand_over, require_env

from elide_ner.runtime import config

logger = logging.getLogger("nvisy.ner.warm")


def main() -> None:
    from huggingface_hub import snapshot_download

    logging.basicConfig(
        level=os.getenv("LOG_LEVEL", "INFO").upper(),
        format="%(asctime)s %(levelname)s %(name)s: %(message)s",
    )
    cache = require_env("HF_HOME")
    model = config.NerSettings().model_id
    logger.info("warming %s", model)
    snapshot_download(model)
    hand_over(cache)
    logger.info("warm")


if __name__ == "__main__":
    main()
