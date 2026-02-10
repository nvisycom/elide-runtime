# Nvisy Runtime — Development

**Technology choices and development roadmap.**

---

## Technology Choices

| Concern | Choice | Rationale |
|---------|--------|-----------|
| Language | Rust | Performance, memory safety, zero-cost abstractions |
| Python extensions | PyO3 | AI/ML model inference where Python ecosystem dominates |
| Async runtime | Tokio | Industry-standard async I/O for Rust |
| HTTP framework | Axum | Tower-based, ergonomic, high performance |
| Serialization | Serde | De facto standard for Rust serialization |
| Graph library | Petgraph | DAG construction, cycle detection, topological sort |
| OpenAPI | utoipa | Compile-time OpenAPI spec generation |
| JSON Schema | schemars | Derive-based JSON Schema for all types |
| Testing | cargo test | Built-in test framework |
| Linting | clippy | Standard Rust linter |
| Formatting | rustfmt | Standard Rust formatter |
| Build | Cargo workspaces | Monorepo management |
| CI | GitHub Actions | Rust toolchain with cargo check, clippy, test, build |
| Python packaging | uv | Fast Python package management |
| Container | Docker | Multi-stage Rust build with Python runtime |

---

## Development Roadmap

### Phase 1 — Foundation (complete)

- **`nvisy-core`** — Type system, traits, plugin registry, error handling
- **`nvisy-detect`** — Regex detection, checksum validation, policy evaluation, redaction
- **`nvisy-engine`** — Graph schema, DAG compiler, executor, run management
- **`nvisy-object`** — S3 object storage connector
- **`nvisy-python`** — PyO3 bridge, AI NER actions
- **`nvisy-server`** — Axum server, REST API, middleware
- **`nvisy-ai`** — Python LLM-based NER
- **`nvisy-exif`** — Python EXIF metadata handling

### Phase 2 — Breadth

- Additional detection patterns (IBAN, passport, driver's license)
- Image-based detection (face detection, license plates, document OCR)
- Additional storage connectors (GCS, Azure Blob)
- SQL connectors (PostgreSQL, MySQL) for audit persistence
- Webhook-based event triggers

### Phase 3 — Production Hardening

- Performance benchmarks and optimization
- Backpressure and memory management
- Graceful shutdown and in-flight run draining
- Secret provider integrations (AWS Secrets Manager, HashiCorp Vault)
- Rate limiting per connector
- Resumable execution with checkpoints

### Phase 4 — Ecosystem

- Plugin SDK documentation
- Community connector contribution guide
- Published crates on crates.io
- Dashboard UI for run monitoring and audit inspection
