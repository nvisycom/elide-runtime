# nvisy-server

HTTP server exposing the nvisy `Engine` pipeline via REST endpoints with auto-generated OpenAPI documentation.

## Endpoints

| Method | Path                   | Description                        |
|--------|------------------------|------------------------------------|
| GET    | `/health`              | Liveness probe                     |
| POST   | `/api/v1/execute`      | Run the full redaction pipeline    |
| POST   | `/api/v1/content`      | Upload content for processing      |
| GET    | `/api/v1/content/:id`  | Download previously uploaded content |
| POST   | `/api/v1/redaction`    | Run redaction on uploaded content  |
| GET    | `/api/v1/analytics`    | Aggregate pipeline analytics       |
| GET    | `/api/v1/openapi.json` | OpenAPI 3.x specification          |
| GET    | `/docs`                | Scalar interactive API reference   |

## Configuration

| Variable              | Default     | Description                    |
|-----------------------|-------------|--------------------------------|
| `HOST`                | `0.0.0.0`  | Bind address                   |
| `PORT`                | `8080`      | Bind port                      |
| `RUST_LOG`            | `info`      | Tracing filter directive       |
| `REQUEST_TIMEOUT_SECS`| `300`       | Per-request timeout in seconds |
