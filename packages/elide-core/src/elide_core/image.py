"""Build-time image steps every service's image shares.

Plain shell commands rather than a BentoML ``Image``, so ``elide-core`` stays
free of a BentoML import: each service's ``runtime/image.py`` passes them to
its own ``Image.run``.
"""

from __future__ import annotations

# Apply Debian's security updates over the slim Python base. The base lags the
# archive by days, and every Python version shares it, so without this a CVE
# fixed after the base was published ships in the image and fails the scan
# (which gates on any fixed HIGH or CRITICAL). Runs first, as root, before the
# image switches to the service user.
OS_SECURITY_UPDATES = (
    "apt-get update"
    " && DEBIAN_FRONTEND=noninteractive apt-get upgrade -y --no-install-recommends"
    " && rm -rf /var/lib/apt/lists/*"
)
