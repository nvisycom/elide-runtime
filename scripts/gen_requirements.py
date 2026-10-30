"""Export each service's locked dependencies into a per-service requirements.txt.

BentoML builds each service image from a flat requirements file (the service's
``Image.requirements_file``). We export that file from the service's own
``uv.lock`` so the image installs exactly what was locked, pinned and hashed,
with no resolution at image-build time.

The exports are derived from the lockfiles, not tracked. They were tracked once
and compared byte-for-byte in CI, which broke whenever ``uv`` reformatted a
marker without changing a version or hash.

``Image.requirements_file()`` resolves its path when the service module is
imported, so every Make target that imports one (``services``,
``services-image``, ``services-test``, ``serve-*``) depends on this script.
Run it by hand before invoking ``bentoml`` directly::

    python3 scripts/gen_requirements.py   # or: make requirements
    uv run --project packages/elide-ner bentoml build -f packages/elide-ner/bentofile.yaml .

Standard library only: it shells out to ``uv`` and needs no environment.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PACKAGES = ROOT / "packages"
SERVICES = ("elide-ner", "elide-ocr", "elide-stt")
# Matches the "pytorch-cpu" [[tool.uv.index]] in each torch service's pyproject.
TORCH_CPU_INDEX = "https://download.pytorch.org/whl/cpu"


def export(service: str) -> str:
    out = subprocess.run(
        [
            "uv",
            "export",
            "--locked",
            "--project",
            str(PACKAGES / service),
            "--format",
            "requirements-txt",
            "--no-dev",
            # The header records the command, absolute project path included,
            # which would put the build machine's paths into the image.
            "--no-header",
            # uv writes local packages relative to the project (`-e .`,
            # `-e ../elide-core`), but the image installs from the bento's
            # source root, which mirrors the repository. They are written
            # below with the paths that resolve there.
            "--no-emit-local",
        ],
        capture_output=True,
        text=True,
        check=True,
    )
    # Editable, and both needed: the bundled source is not on the path by
    # itself, so without the service's own entry the container fails at startup
    # with "No module named elide_<service>". The service's `build_include`
    # copies both directories into the image before this file is installed.
    local = f"-e ./packages/elide-core\n-e ./packages/{service}\n"
    # `uv export` emits no index URLs, but the torch pins are `+cpu` local
    # versions published only on the PyTorch CPU index. The in-image
    # `uv pip install -r` reads nothing but this file, so the index has to
    # travel with it or the build fails with "no version of torch==...+cpu".
    #
    # The matching `index-strategy` cannot live here: a requirements file
    # accepts only a few directives, and `--index-strategy` is not one of them.
    # Each torch service's image writes it to /etc/uv/uv.toml instead; see the
    # comment on `.run(...)` in its runtime/image.py.
    #
    # Only where a `+cpu` pin needs it: an image that does not configure that
    # index-strategy (elide-ocr, which has no torch) would otherwise resolve
    # unrelated packages from the PyTorch index's mirrors and fail their hashes.
    index = f"--extra-index-url {TORCH_CPU_INDEX}\n" if "+cpu" in out.stdout else ""
    return index + out.stdout + local


def main(services: list[str]) -> None:
    for service in services or SERVICES:
        path = PACKAGES / service / "requirements.txt"
        path.write_text(export(service))
        print(f"wrote {path.relative_to(ROOT)}")


if __name__ == "__main__":
    main(sys.argv[1:])
