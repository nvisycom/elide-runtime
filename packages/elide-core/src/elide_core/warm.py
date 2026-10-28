"""Shared pieces for a service's cache warmer.

The published images carry no weights, so a deployment warms the cache before
the service starts — see ``docker/compose.yaml``, where each service has a
one-shot ``*-init`` partner that runs its own ``warm`` module and which the
service gates on.

What lives here is only what every warmer needs and what this package can
honestly depend on: reading a required variable, unbuffered logging, and
handing the populated cache to the uid the services run as. The download
itself lives with the service, because each needs a different library
(``huggingface_hub``, ``doctr``, ``faster_whisper``) and this package depends
on none of them.
"""

from __future__ import annotations

import logging
import os
import sys
from pathlib import Path

#: The uid the services run as (``bentoml`` in the BentoML base image).
#:
#: A warmer runs as root because Docker creates a fresh named volume owned by
#: root, which the unprivileged service could not write. Ownership is handed
#: over once the download finishes; the service mounts the volume read-only
#: and never runs as root itself.
SERVICE_UID = 1034
SERVICE_GID = 1034


def configure_logging() -> None:
    """Configure standalone warmer logging without replacing existing handlers."""
    logging.basicConfig(
        level=os.getenv("LOG_LEVEL", "INFO").upper(),
        format="%(asctime)s %(levelname)s %(name)s: %(message)s",
    )


def require_env(name: str) -> str:
    """Read a required environment variable, or exit naming it.

    A missing model id is a deployment error, not an exception worth a
    traceback: the message is the useful part.
    """
    value = os.environ.get(name)
    if not value:
        sys.exit(f"{name} is required to warm the cache, and is unset or empty.")
    return value


def log(message: str) -> None:
    """Print unbuffered.

    Compose interleaves this with its own progress output, and a buffered warm
    looks indistinguishable from a hang while a cold cache pulls gigabytes.
    """
    print(message, flush=True)


def hand_over(cache: str) -> None:
    """Give ``cache`` and everything under it to the service's uid.

    Root created these files. Without this the service cannot read its own
    cache and fails at startup with a ``PermissionError`` that looks nothing
    like a permissions problem.

    Uses ``lchown``, not ``chown``: the Hugging Face cache keeps weights in
    ``blobs/`` and puts symlinks to them in ``snapshots/``. ``chown`` follows
    a symlink and changes the blob instead of the link, so whether a given
    path ended up owned by the service depended on which of the two
    ``rglob`` reached first — measured as 12 of 61 paths left root-owned,
    enough for the service to fail reading its own cache. ``lchown`` changes
    the link itself, and the blob is changed when ``rglob`` reaches it
    directly.
    """
    root = Path(cache)
    for path in (root, *root.rglob("*")):
        os.lchown(path, SERVICE_UID, SERVICE_GID)
