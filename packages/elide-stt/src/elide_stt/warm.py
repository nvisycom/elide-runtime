"""Download the transcription model into the cache, then exit.

Run by this service's one-shot init container before the service starts::

    python -m elide_stt.warm

Also fetches the diarization model when enabled. That is opt-in because the
model is **gated** on the Hub: it cannot be redistributed in an image, and
fetching it needs a token belonging to an account that has accepted its
terms. Shared pieces come from ``elide_core.warm``.
"""

from __future__ import annotations

import os
import sys

from elide_core.warm import hand_over, log, require_env

from elide_stt import config


def main() -> None:
    from faster_whisper.utils import download_model

    cache = require_env("HF_HOME")
    model = require_env("ELIDE_BENTO_MODEL_NAME")

    # Validate the whole configuration before downloading anything. The
    # transcription model alone is over a gigabyte, and failing after it on a
    # token that was readable from the start wastes the download and the
    # operator's time.
    diarize = ""
    token = ""
    if config.diarize():
        diarize = require_env("ELIDE_BENTO_STT_DIARIZE_MODEL")
        token = os.environ.get("ELIDE_BENTO_STT_HF_TOKEN") or ""
        if not token:
            sys.exit(
                "ELIDE_BENTO_STT_DIARIZE=true needs ELIDE_BENTO_STT_HF_TOKEN.\n"
                "The model is gated: accept its terms at\n"
                f"https://huggingface.co/{diarize}\n"
                "then set the token to one belonging to that account."
            )

    log(f"warming {model}")
    download_model(model)

    if diarize:
        from huggingface_hub import snapshot_download

        log(f"warming {diarize} (gated)")
        snapshot_download(diarize, token=token)

    hand_over(cache)
    log("warm")


if __name__ == "__main__":
    main()
