# Testcontainers-only image: real BentoML applications with deterministic model
# fixtures. Python matches the workspace constraint; uv matches the CI pin.
FROM python:3.12-slim-bookworm
RUN pip install --no-cache-dir uv==0.13.5

WORKDIR /app

# Copy manifests before sources so application/test edits reuse the dependency
# layer. uv needs every workspace member's manifest to validate the lockfile.
COPY pyproject.toml uv.lock ./
COPY packages/elide-core/pyproject.toml packages/elide-core/pyproject.toml
COPY packages/elide-ner/pyproject.toml packages/elide-ner/pyproject.toml
COPY packages/elide-ocr/pyproject.toml packages/elide-ocr/pyproject.toml
COPY packages/elide-stt/pyproject.toml packages/elide-stt/pyproject.toml

# Install HTTP and decoding dependencies only. The E2E group excludes model
# runtimes and weights; server.py supplies the model-library fixtures.
RUN uv sync --locked --only-group e2e

COPY packages/ packages/
# The Rust build context places the test launcher at this path.
COPY server.py tests/server.py

# Import the checkout directly without installing production model extras.
# BENTOML_CONTAINERIZED skips build-time image/requirements evaluation, offline
# flags prevent Hub downloads, and /dev/null rules out mounted-weight overrides.
ENV PYTHONPATH=/app/packages/elide-core/src:/app/packages/elide-ner/src:/app/packages/elide-ocr/src:/app/packages/elide-stt/src \
    BENTOML_CONTAINERIZED=1 \
    HF_HUB_OFFLINE=1 \
    TRANSFORMERS_OFFLINE=1 \
    ELIDE_BENTO_MODEL_PATH=/dev/null

# Each service listens on 3000; Testcontainers assigns a free host port and waits
# for /readyz. Its command appends the task name (ner, ocr, or stt) to the launcher.
EXPOSE 3000
ENTRYPOINT ["/app/.venv/bin/python", "/app/tests/server.py"]
