# syntax=docker/dockerfile:1
#
# Multi-stage build for the `nvisy` binary (shipped by nvisy-cli).
#
#   docker build -f docker/Dockerfile -t nvisy-server .
#
# Build context must be the repository root.

# Stage 1: Build
FROM rust:1.92-bookworm AS build

RUN apt-get update \
    && apt-get install -y --no-install-recommends python3-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Forward-compatible PyO3 ABI for any Python ≥ 3.7.
ENV PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1

# Dependency cache layer
# Copy only manifests + lock so dependency builds are cached until a
# Cargo.toml or Cargo.lock changes. Stub source files satisfy cargo.
COPY Cargo.toml Cargo.lock ./
COPY crates/nvisy-cli/Cargo.toml crates/nvisy-cli/Cargo.toml
COPY crates/nvisy-codec/Cargo.toml crates/nvisy-codec/Cargo.toml
COPY crates/nvisy-core/Cargo.toml crates/nvisy-core/Cargo.toml
COPY crates/nvisy-engine/Cargo.toml crates/nvisy-engine/Cargo.toml
COPY crates/nvisy-identify/Cargo.toml crates/nvisy-identify/Cargo.toml
COPY crates/nvisy-ocr/Cargo.toml crates/nvisy-ocr/Cargo.toml
COPY crates/nvisy-ontology/Cargo.toml crates/nvisy-ontology/Cargo.toml
COPY crates/nvisy-pattern/Cargo.toml crates/nvisy-pattern/Cargo.toml
COPY crates/nvisy-python/Cargo.toml crates/nvisy-python/Cargo.toml
COPY crates/nvisy-rig/Cargo.toml crates/nvisy-rig/Cargo.toml
COPY crates/nvisy-server/Cargo.toml crates/nvisy-server/Cargo.toml

# Stub lib.rs / main.rs + empty READMEs (every crate uses
# `include_str!("../README.md")`).
RUN set -e; \
    for c in nvisy-codec nvisy-core nvisy-engine nvisy-identify \
             nvisy-ocr nvisy-ontology nvisy-pattern nvisy-python \
             nvisy-rig nvisy-server; do \
        mkdir -p "crates/$c/src"; \
        echo "" > "crates/$c/src/lib.rs"; \
        touch "crates/$c/README.md"; \
    done; \
    mkdir -p crates/nvisy-cli/src; \
    echo "fn main() {}" > crates/nvisy-cli/src/main.rs; \
    touch crates/nvisy-cli/README.md

RUN cargo build --release 2>/dev/null || true

# Full build
COPY . .
RUN cargo build --release --bin nvisy

# Stage 2: Runtime
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        python3 \
        python3-pip \
        python3-venv \
    && rm -rf /var/lib/apt/lists/*

# PDFium (PDF-to-image rendering)
COPY scripts/install-pdfium.sh /tmp/install-pdfium.sh
RUN /tmp/install-pdfium.sh && rm /tmp/install-pdfium.sh

# Python packages
COPY packages/ /opt/nvisy/packages/
RUN python3 -m pip install --break-system-packages --no-cache-dir \
    /opt/nvisy/packages/nvisy-ai \
    /opt/nvisy/packages/nvisy-exif

# Binary
COPY --from=build /app/target/release/nvisy /usr/local/bin/

EXPOSE 8080
ENTRYPOINT ["nvisy"]
