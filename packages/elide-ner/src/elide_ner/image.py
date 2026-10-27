"""Build-time image specification for the NER service.

Separate from the service because it is *build* configuration that merely has
to be importable at runtime: `requirements_file` reads the file when this
module is imported, and the path it needs differs between a build and the
container, so the spec is skipped inside the image rather than evaluated. None
of it concerns inference, and 50 lines about `uv pip install` sat between a
reader and the handler.
"""

from __future__ import annotations

import os

import bentoml

# BentoML builds the image from this config (`bentoml build` + `containerize`);
# no hand-written Dockerfile. The requirements file is exported per-service from
# the workspace lock by `make services` (scripts/gen_requirements.py), not
# committed; bundled source is scoped by
# bentofile.yaml's `include`. lock_python_packages=False: the file is already
# locked + hashed, so BentoML must not re-resolve it.
# Build-time image spec, skipped inside the container. `requirements_file`
# reads the file when this module is imported, and the path it needs differs
# between the two contexts (`packages/...` at build time, `src/packages/...`
# under the bento root), so evaluating it at runtime raises FileNotFoundError
# and the service never starts. BENTOML_CONTAINERIZED is set in the image and
# unset during a build, which is the distinction.
#
# build_include copies the bundled workspace source to ./src *before* the
# image's `uv pip install`, which is what makes the editable path dependencies
# in requirements.txt resolvable: the install then runs with
# INSTALL_ROOT=./src, where `./packages/elide-*` are real directories.
#
# lock_python_packages=False: the file is already locked and hashed by
# scripts/gen_requirements.py, so BentoML must not re-resolve it.
image = (
    None
    if os.getenv("BENTOML_CONTAINERIZED")
    else (
        bentoml.images.Image(python_version="3.12", lock_python_packages=False)
        # The torch pins are `+cpu` builds from the PyTorch CPU index, which
        # requirements.txt carries as an --extra-index-url. That index also mirrors
        # unrelated packages (certifi, filelock, ...) at versions that may not match
        # ours, and uv defaults to `first-index`: it refuses to look past the first
        # index that carries a name at all, so the install fails on those.
        #
        # `--index-strategy` is not a directive a requirements file accepts, and a
        # RUN cannot export an env var to the next layer, so the setting goes in
        # uv's system config, which uv reads with no further configuration. This
        # runs before the install step.
        #
        # "unsafe" describes resolution, not installation: every pin in
        # requirements.txt is hash-locked, so an artifact from any index is
        # rejected unless its SHA-256 matches the lockfile. The hashes, not the
        # index order, are what make this safe.
        .run(
            "mkdir -p /etc/uv && printf 'index-strategy = \"unsafe-best-match\"\\n'"
            " > /etc/uv/uv.toml"
        )
        .build_include("packages", "pyproject.toml", "uv.lock")
        .requirements_file("packages/elide-ner/requirements.txt")
    )
)
