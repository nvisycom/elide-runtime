# nvisy-server

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

HTTP server exposing the nvisy `Engine` pipeline via REST endpoints with auto-generated OpenAPI documentation.

Built on [Axum](https://docs.rs/axum) with [Aide](https://docs.rs/aide) for OpenAPI spec generation and [Scalar](https://github.com/scalar/scalar) for interactive API docs.

## Feature Flags

Vendor features are forwarded from [`nvisy-engine`](../nvisy-engine). All are disabled by default.

| Feature | Default | Description |
|---------|---------|-------------|
| `openai` | no | Enable all OpenAI providers (GPT, Whisper STT) |
| `anthropic` | no | Enable Anthropic Claude completion provider |
| `google` | no | Enable Google Gemini + Google Cloud Vision OCR |
| `microsoft` | no | Enable Azure Document Intelligence OCR |
| `amazon` | no | Enable AWS Textract OCR |

## Endpoints

| Method | Path                   | Handler              | Description                          |
|--------|------------------------|----------------------|--------------------------------------|
| GET    | `/health`              | `check::health`      | Liveness probe                       |
| POST   | `/api/v1/execute`      | `execute::execute`   | Run the full redaction pipeline      |
| POST   | `/api/v1/ingest`       | `ingest::upload`     | Upload content (multipart form data) |
| GET    | `/api/v1/ingest/{id}`  | `ingest::download`   | Download previously uploaded content |
| POST   | `/api/v1/redaction`    | `redact::redact`     | Run redaction on uploaded content    |
| GET    | `/api/v1/analytics`    | `check::analytics`   | Aggregate pipeline analytics         |
| GET    | `/api/v1/openapi.json` | (specification)      | OpenAPI 3.x specification            |
| GET    | `/docs`                | Scalar               | Interactive API reference            |

## Middleware

Applied in order (outermost first):

1. **Specification**: finalises the aide route tree, serves OpenAPI JSON and Scalar UI.
2. **Recovery**: catches panics and enforces per-request timeouts.
3. **Observability**: assigns/propagates `x-request-id`, structured tracing, sensitive header redaction.
4. **Security**: permissive CORS policy and request body size limits.
5. **Compression**: gzip, brotli, and zstd response compression.

## Configuration

All options are available as CLI flags and environment variables:

| Flag                      | Env                    | Default      | Description                    |
|---------------------------|------------------------|--------------|--------------------------------|
| `--host`                  | `HOST`                 | `0.0.0.0`    | Bind address                   |
| `--port`                  | `PORT`                 | `8080`       | Bind port                      |
| `--log-level`             | `RUST_LOG`             | `info`       | Tracing filter directive       |
| `--content-dir`           | `CONTENT_DIR`          | (temp dir)   | Temporary content storage      |
| `--body-limit-bytes`      | `BODY_LIMIT_BYTES`     | `52428800`   | Max request body size (50 MiB) |
| `--request-timeout-secs`  | `REQUEST_TIMEOUT_SECS` | `300`        | Per-request timeout in seconds |

## Documentation

See [`docs/`](../../docs/) for architecture, security, and API documentation.

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
