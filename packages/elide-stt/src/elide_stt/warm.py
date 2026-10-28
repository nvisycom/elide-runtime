"""Download the transcription model into the cache, then exit.

Run by this service's one-shot init container before the service starts::

    python -m elide_stt.warm

Also fetches the diarization model when enabled. That is opt-in because the
model is **gated** on the Hub: it cannot be redistributed in an image, and
fetching it needs a token belonging to an account that has accepted its
terms. Shared pieces come from ``elide_core.warm``.
"""

from __future__ import annotations

import logging
import os
import sys

from elide_core.warm import hand_over, require_env

from elide_stt.runtime import config

logger = logging.getLogger("nvisy.stt.warm")


def main() -> None:
    from faster_whisper.utils import download_model

    logging.basicConfig(
        level=os.getenv("LOG_LEVEL", "INFO").upper(),
        format="%(asctime)s %(levelname)s %(name)s: %(message)s",
    )
    cache = require_env("HF_HOME")
    settings = config.SttSettings()
    model = settings.model_name

    # Validate the whole configuration before downloading anything. The
    # transcription model alone is over a gigabyte, and failing after it on a
    # token that was readable from the start wastes the download and the
    # operator's time.
    diarize = ""
    token = ""
    if settings.diarize:
        diarize = settings.diarize_model
        token = settings.hf_token.get_secret_value() if settings.hf_token else ""
        if not token:
            sys.exit(
                "ELIDE_BENTO_STT_DIARIZE=true needs ELIDE_BENTO_STT_HF_TOKEN.\n"
                "The model is gated: accept its terms at\n"
                f"https://huggingface.co/{diarize}\n"
                "then set the token to one belonging to that account."
            )

    logger.info("warming %s", model)
    download_model(model)

    if diarize:
        from huggingface_hub import snapshot_download

        logger.info("warming %s (gated)", diarize)
        snapshot_download(diarize, token=token)

    hand_over(cache)
    logger.info("warm")


if __name__ == "__main__":
    main()
