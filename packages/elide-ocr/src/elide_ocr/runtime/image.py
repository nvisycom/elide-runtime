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
# build_include copies the bundled source to ./src *before* the image's
# `uv pip install`, which is what makes the editable path dependencies in
# requirements.txt resolvable: the install then runs with INSTALL_ROOT=./src,
# where `./packages/elide-core` and `./packages/elide-ocr` are real directories.
#
# lock_python_packages=False: the file is already locked and hashed by
# scripts/gen_requirements.py, so BentoML must not re-resolve it.
image = (
    None
    if os.getenv("BENTOML_CONTAINERIZED")
    else (
        bentoml.images.Image(python_version="3.12", lock_python_packages=False)
        # No torch, so no PyTorch CPU index and none of the index-strategy
        # configuration the other services' images need for it: everything in
        # requirements.txt resolves from PyPI.
        .build_include("packages/elide-core", "packages/elide-ocr")
        .requirements_file("packages/elide-ocr/requirements.txt")
    )
)
