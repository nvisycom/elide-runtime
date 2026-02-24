# nvisy-server

HTTP server exposing the nvisy `Engine` pipeline via REST endpoints with auto-generated OpenAPI documentation.

Built on [Axum](https://docs.rs/axum) with [Aide](https://docs.rs/aide) for OpenAPI spec generation and [Scalar](https://github.com/scalar/scalar) for interactive API docs.

## Endpoints

| Method | Path                   | Handler                | Description                          |
|--------|------------------------|------------------------|--------------------------------------|
| GET    | `/health`              | `check::health`        | Liveness probe                       |
| POST   | `/api/v1/execute`      | `execute::execute`     | Run the full redaction pipeline      |
| POST   | `/api/v1/ingest`      | `ingest::upload`       | Upload content (multipart form data) |
| GET    | `/api/v1/ingest/{id}` | `ingest::download`     | Download previously uploaded content |
| POST   | `/api/v1/redaction`    | `redact::redact`       | Run redaction on uploaded content    |
| GET    | `/api/v1/analytics`    | `check::analytics`     | Aggregate pipeline analytics         |
| GET    | `/api/v1/openapi.json` | `execute::openapi_json`| OpenAPI 3.x specification            |
| GET    | `/docs`                | Scalar                 | Interactive API reference            |

## Architecture

```
src/
├── main.rs               # Tokio entry, tracing init, graceful shutdown + cleanup
├── service/mod.rs         # ServiceState, DI macro, router construction
├── middleware/mod.rs       # tower-http layers (tracing, CORS, timeout, request-id,
│                          #   body limit, rate limit, compression)
└── handler/
    ├── mod.rs             # Route tree with OpenAPI operation metadata
    ├── check.rs           # GET  /health, GET /api/v1/analytics
    ├── ingest.rs          # POST /api/v1/ingest, GET /api/v1/ingest/{id}
    ├── execute.rs         # POST /api/v1/execute, GET /api/v1/openapi.json
    ├── redact.rs          # POST /api/v1/redaction
    ├── request/           # Typed request bodies (ExecuteRequest, RedactionRequest)
    └── response/          # Typed response bodies + error handling (ServerError)
```

## Middleware

Applied in order (outermost first):

1. **Request ID**: assigns and propagates `x-request-id` header
2. **Body limit**: rejects oversized request bodies
3. **Timeout**: per-request deadline with 504 on expiry
4. **CORS**: permissive cross-origin policy
5. **Tracing**: structured request/response logging
6. **Compression**: gzip, brotli, and zstd response compression

## Configuration

| Variable                   | Default      | Description                        |
|----------------------------|--------------|------------------------------------|
| `HOST`                     | `0.0.0.0`   | Bind address                       |
| `PORT`                     | `8080`       | Bind port                          |
| `RUST_LOG`                 | `info`       | Tracing filter directive           |
| `REQUEST_TIMEOUT_SECS`     | `300`        | Per-request timeout in seconds     |
| `REQUEST_BODY_LIMIT_BYTES` | `52428800`   | Max request body size (50 MiB)     |
