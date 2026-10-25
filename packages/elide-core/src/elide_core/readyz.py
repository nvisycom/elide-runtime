"""Container readiness probe.

Run as the healthcheck for each service image:

    python -m elide_core.readyz

Exits 0 once ``/readyz`` answers, non-zero with a one-line reason otherwise.
Docker keeps that line in ``.State.Health.Log[].Output``, so

    docker inspect --format '{{json .State.Health}}' elide-ner

says *why* a container is unhealthy instead of leaving an operator to infer it
from an exit code.

The reasons are distinguished because they need opposite responses and look
identical from ``docker ps``:

``loading``
    The service is up but not serving yet. BentoML binds the port before the
    model finishes loading in ``__init__``, so a probe usually sees a read
    timeout rather than a status — measured at 2 of the 4 seconds a cold
    ``elide-ner`` takes to answer. Wait.

``down``
    Nothing is listening: the process died, or is crash-looping. Read the
    logs.

Lives here rather than inline in the healthcheck because a probe is code: it
has branches that each needed checking against a real container, and the
timeout case was missed on the first attempt precisely because it was a YAML
string nothing could exercise.
"""

from __future__ import annotations

import sys
import urllib.error
import urllib.request

# The in-container port, which is fixed; compose maps it outward per service.
URL = "http://localhost:3000/readyz"

# Under compose's `timeout: 5s`, so a slow answer is reported by this script
# rather than killed by Docker — a killed probe has no output to show.
TIMEOUT = 4.0


def main() -> None:
    try:
        urllib.request.urlopen(URL, timeout=TIMEOUT)
    except urllib.error.HTTPError as error:
        # A status means the server answered, so it is up but not ready.
        sys.exit(f"loading: /readyz returned HTTP {error.code}")
    except TimeoutError:
        # Checked before URLError: the connection was accepted and then left
        # unanswered, which is the normal state while the model loads. Without
        # this branch it escapes as a traceback.
        sys.exit(f"loading: accepted the connection but did not answer in {TIMEOUT:.0f}s")
    except urllib.error.URLError as error:
        sys.exit(f"down: no server on :3000 ({error.reason})")


if __name__ == "__main__":
    main()
