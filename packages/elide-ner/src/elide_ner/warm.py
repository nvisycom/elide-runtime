"""Download the NER model into the Hugging Face cache, then exit.

Run by this service's one-shot init container before the service starts::

    python -m elide_ner.warm

Lives with the service rather than in ``elide-core`` because it imports
``huggingface_hub``, which only this service and ``elide-stt`` depend on.
Shared pieces — the env helper, logging, the ownership handover — come from
``elide_core.warm``.
"""

from __future__ import annotations

from elide_core.warm import hand_over, log, require_env

from elide_ner import config


def main() -> None:
    from huggingface_hub import snapshot_download

    cache = require_env("HF_HOME")
    model = config.model_id()
    log(f"warming {model}")
    snapshot_download(model)
    hand_over(cache)
    log("warm")


if __name__ == "__main__":
    main()
