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
# Matches [[tool.uv.index]] name = "pytorch-cpu" in the root pyproject.toml.
TORCH_CPU_INDEX = "https://download.pytorch.org/whl/cpu"

SERVICES = {
    "elide-ner": ROOT / "packages" / "elide-ner" / "requirements.txt",
    "elide-ocr": ROOT / "packages" / "elide-ocr" / "requirements.txt",
    "elide-stt": ROOT / "packages" / "elide-stt" / "requirements.txt",
}


def export(package: str) -> str:
    # Both workspace packages are emitted as editable path dependencies — the
    # service itself and elide-core — and each service's `build_include` copies
    # that source into the image before the install step, so the paths resolve.
    #
    # `--no-emit-project` is deliberately NOT passed: without the service's own
    # editable entry nothing installs it, and the container fails at startup
    # with "No module named elide_<service>". BentoML does not add the bundled
    # source to the path by itself.
    out = subprocess.run(
        [
            "uv",
            "export",
            "--locked",
            "--package",
            package,
            "--format",
            "requirements-txt",
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    # `uv export` emits no index URLs, but the torch pins are `+cpu` local
    # versions published only on the PyTorch CPU index (configured as an
    # explicit index in the root pyproject.toml). The in-image
    # `uv pip install -r` reads nothing but this file, so the index has to
    # travel with it or the build fails with "no version of torch==...+cpu".
    #
    # The matching `index-strategy` cannot live here — a requirements file
    # accepts only a few directives, and `--index-strategy` is not one of them
    # — so each service's image writes it to /etc/uv/uv.toml instead. See the
    # comment on `.run(...)` in the service modules.
    return f"--extra-index-url {TORCH_CPU_INDEX}\n{out.stdout}"


def main() -> None:
    for package, path in SERVICES.items():
        path.write_text(export(package))
        print(f"wrote {path.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
