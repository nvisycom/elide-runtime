"""Export each service's locked dependencies into a per-service requirements.txt.

BentoML builds each service image from a flat requirements file (the service's
``Image.requirements_file``). We export that file per service from the single
workspace ``uv.lock`` so the image installs exactly the service's resolved
dependency subtree (including the editable ``nvisy-core``), pinned and hashed —
no resolution at image-build time.

Run::

    uv run python scripts/gen_requirements.py          # write files
    uv run python scripts/gen_requirements.py --check   # fail if out of date (CI)
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
# service package -> where its requirements.txt is written (next to pyproject).
SERVICES = {
    "nvisy-ocr": ROOT / "packages" / "nvisy-ocr" / "requirements.txt",
    "nvisy-vl": ROOT / "packages" / "nvisy-vl" / "requirements.txt",
    "nvisy-ner": ROOT / "packages" / "nvisy-ner" / "requirements.txt",
}


def export(package: str) -> str:
    # --no-emit-project drops the service itself (BentoML installs the source);
    # nvisy-core stays as an editable path dep, its source bundled via
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


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="Fail if any file is stale.")
    args = parser.parse_args()

    stale: list[str] = []
    for package, path in SERVICES.items():
        generated = export(package)
        if args.check:
            current = path.read_text() if path.exists() else ""
            if current != generated:
                stale.append(package)
        else:
            path.write_text(generated)
            print(f"wrote {path.relative_to(ROOT)}")

    if args.check and stale:
        print(
            f"requirements out of date: {', '.join(stale)}. "
            "Run `uv run python scripts/gen_requirements.py`.",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
