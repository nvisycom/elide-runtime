"""Export each service's locked dependencies into a per-service requirements.txt.

BentoML builds each service image from a flat requirements file (the service's
``Image.requirements_file``). We export that file per service from the single
workspace ``uv.lock`` so the image installs exactly the service's resolved
dependency subtree (including the editable ``bento-core``), pinned and hashed —
no resolution at image-build time.

The exports are derived from the lockfile, not tracked: ``make services`` runs
this before building, so an image is always built from the lock as committed.
They were tracked once and compared byte-for-byte in CI, which broke whenever
``uv`` reformatted a marker without changing a version or hash.

Run::

    uv run python scripts/gen_requirements.py
"""

from __future__ import annotations

import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
# service package -> where its requirements.txt is written (next to pyproject).
SERVICES = {
    "bento-doctr": ROOT / "packages" / "bento-doctr" / "requirements.txt",
    "bento-paddleocr": ROOT / "packages" / "bento-paddleocr" / "requirements.txt",
    "bento-gliner2": ROOT / "packages" / "bento-gliner2" / "requirements.txt",
    "bento-whisper": ROOT / "packages" / "bento-whisper" / "requirements.txt",
}


def export(package: str) -> str:
    # --no-emit-project drops the service itself (BentoML installs the source);
    # bento-core stays as an editable path dep, its source bundled via
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
