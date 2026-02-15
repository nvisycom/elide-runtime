FROM rust:1.85-bookworm AS builder

RUN apt-get update && apt-get install -y python3-dev python3-pip && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy manifests first to cache dependency builds
COPY Cargo.toml Cargo.lock ./
COPY crates/nvisy-core/Cargo.toml crates/nvisy-core/Cargo.toml
COPY crates/nvisy-detect/Cargo.toml crates/nvisy-detect/Cargo.toml
COPY crates/nvisy-engine/Cargo.toml crates/nvisy-engine/Cargo.toml
COPY crates/nvisy-object/Cargo.toml crates/nvisy-object/Cargo.toml
COPY crates/nvisy-python/Cargo.toml crates/nvisy-python/Cargo.toml
COPY crates/nvisy-server/Cargo.toml crates/nvisy-server/Cargo.toml

# Create empty src files to satisfy cargo's manifest checks
RUN for crate in nvisy-core nvisy-detect nvisy-engine nvisy-object nvisy-python; do \
      mkdir -p crates/$crate/src && echo "" > crates/$crate/src/lib.rs; \
    done && \
    mkdir -p crates/nvisy-server/src && echo "fn main() {}" > crates/nvisy-server/src/main.rs

# Create stub READMEs for crates that use doc = include_str!("../README.md")
RUN for crate in nvisy-core nvisy-detect nvisy-engine nvisy-object nvisy-python nvisy-server; do \
      touch crates/$crate/README.md; \
    done

ENV PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1

# Cache dependency build
RUN cargo build --release 2>/dev/null || true

# Copy full source and build
COPY . .
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y \
    python3 python3-pip python3-venv ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Install Python packages
COPY packages/ /opt/nvisy/packages/
RUN python3 -m pip install --break-system-packages \
    /opt/nvisy/packages/nvisy-ai \
    /opt/nvisy/packages/nvisy-exif

COPY --from=builder /app/target/release/nvisy-server /usr/local/bin/nvisy-server

EXPOSE 8080
CMD ["nvisy-server"]
