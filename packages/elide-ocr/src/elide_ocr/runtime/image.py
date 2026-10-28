"""Build-time image specification for the OCR service."""

import os

import bentoml

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
        .system_packages("libgl1", "libglib2.0-0")
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
        .requirements_file("packages/elide-ocr/requirements.txt")
    )
)
