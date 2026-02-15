# Nvisy Runtime

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

A data protection runtime for AI pipelines — detect, redact, and audit sensitive data across documents, images, and streams.

Built in Rust with Python extensions for AI-powered detection.

## Workspace

```
crates/
  nvisy-core/        Types, traits, plugin registry, error handling
  nvisy-detect/      Regex patterns, policy evaluation, redaction actions
  nvisy-engine/      DAG graph compiler and execution engine
  nvisy-object/      Object storage connectors (S3)
  nvisy-python/      Python interop for AI-powered NER via PyO3
  nvisy-server/      Axum HTTP server with REST API

packages/
  nvisy-ai/          Python: LLM-based entity detection
  nvisy-exif/        Python: EXIF metadata reading and stripping
```

## Quick Start

```bash
cargo build --workspace
cargo test --workspace
cargo run -p nvisy-server
```

## Development

```bash
make dev      # cargo-watch dev server
make ci       # lint + check + test + build
make help     # list all targets
```

## Documentation

See [`docs/`](docs/) for architecture and development documentation.

## License

Apache 2.0 License, see [LICENSE.txt](LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
