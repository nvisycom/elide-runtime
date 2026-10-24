"""Export each service's locked dependencies into a per-service requirements.txt.

BentoML builds each service image from a flat requirements file (the service's
``Image.requirements_file``). We export that file per service from the single
workspace ``uv.lock`` so the image installs exactly the service's resolved
dependency subtree (including the editable ``elide-core``), pinned and hashed —
no resolution at image-build time.

The exports are derived from the lockfile, not tracked. They were tracked once
and compared byte-for-byte in CI, which broke whenever ``uv`` reformatted a
marker without changing a version or hash.

``Image.requirements_file()`` resolves its path when the service module is
imported, so every Make target that imports one (``services``,
``services-image``, ``services-test``, ``serve-*``) depends on this script.
Run it by hand before invoking ``bentoml`` directly::

    uv run python scripts/gen_requirements.py   # or: make requirements
    uv run bentoml build -f packages/elide-ner/bentofile.yaml .
"""

from __future__ import annotations

import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
# service package -> where its requirements.txt is written (next to pyproject).
SERVICES = {
    "elide-ner": ROOT / "packages" / "elide-ner" / "requirements.txt",
    "elide-ocr": ROOT / "packages" / "elide-ocr" / "requirements.txt",
    "elide-stt": ROOT / "packages" / "elide-stt" / "requirements.txt",
}


def export(package: str) -> str:
    # --no-emit-project drops the service itself (BentoML installs the source);
    # elide-core stays as an editable path dep, its source bundled via
    # Image.build_include.
    out = subprocess.run(
        [
            "uv",
            "export",
            "--locked",
            "--package",
            package,
            "--no-emit-project",
            "--format",
            "requirements-txt",
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    return out.stdout


def main() -> None:
    for package, path in SERVICES.items():
        path.write_text(export(package))
        print(f"wrote {path.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
