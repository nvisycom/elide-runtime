# nvisy-server

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

HTTP server exposing the nvisy document pipeline over a REST API
with auto-generated OpenAPI documentation.

Built on [Axum](https://docs.rs/axum) with [Aide](https://docs.rs/aide)
for OpenAPI spec generation and
[Scalar](https://github.com/scalar/scalar) for interactive API docs.

## Feature Flags

Forwarded from [`nvisy-engine`](../nvisy-engine). All are off
by default; the CLI entry point opts them in.

| Feature | Default | Description |
|---------|---------|-------------|
| `tabular` | yes | CSV + XLSX |
| `image` | yes | PNG, JPEG, TIFF + OCR + VLM detection |
| `audio` | yes | WAV, MP3 + STT |
| `rich` | yes | PDF, DOCX |
| `openai` | no | OpenAI GPT completion provider |
| `anthropic` | no | Anthropic Claude completion provider |
| `google` | no | Google Gemini completion provider |
| `bento` | no | Externalised inference backends (BentoML NER + OCR) |

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Liveness probe |
| GET | `/api/v1/analytics` | Aggregate pipeline analytics |
| GET / POST | `/api/v1/contexts[/{id}]` | Manage reference contexts (CRUD) |
| GET / POST | `/api/v1/files[/{id}]` | Upload and download files |
| GET / POST | `/api/v1/policies[/{id}]` | Manage redaction policies (CRUD) |
| GET / POST | `/api/v1/runs[/{id}[/cancel]]` | Create / fetch / cancel pipeline runs |
| GET | `/api/v1/openapi.json` | OpenAPI 3.x specification |
| GET | `/docs` | Interactive Scalar API reference |

## Middleware

Composed via extension traits on `ApiRouter` / `Router`. Layers
applied outside-in:

1. **Recovery** — catches panics, enforces per-request timeouts.
2. **Observability** — assigns / propagates `x-request-id`,
   structured tracing, sensitive header redaction.
3. **Security** — CORS policy, request body size limits, gzip /
   brotli / zstd response compression.
4. **OpenAPI** — finalises the aide route tree, serves the spec
   JSON and the Scalar UI.

## Configuration

CLI flags + env vars live on the [`nvisy-cli`](../nvisy-cli)
binary; see its README for the full table. Default bind is
`0.0.0.0:8080`.

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
