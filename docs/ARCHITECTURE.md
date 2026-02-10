# Nvisy Runtime — Architecture

**Technical architecture for the Nvisy Runtime data protection platform.**

---

## 1. Overview

Nvisy Runtime is a Rust-native, DAG-based data protection platform. It detects, classifies, and redacts sensitive data across documents, images, and streams. The system is structured as a Cargo workspace of composable crates, with Python extensions for AI-powered detection.

---

## 2. Crate Structure

```
crates/
  nvisy-core/        Core types, traits, plugin registry, errors
  nvisy-detect/      Regex patterns, checksum validation, policy evaluation, redaction
  nvisy-engine/      Graph schema, DAG compiler, executor, run management
  nvisy-object/      Object storage client and connectors (S3)
  nvisy-python/      PyO3 bridge for Python AI modules
  nvisy-server/      Axum HTTP server, handlers, middleware

packages/
  nvisy-ai/          Python: LLM-based NER detection
  nvisy-exif/        Python: EXIF metadata reading/stripping
```

### Dependency graph

```
              nvisy-server
              /     |     \
             ▼      ▼      ▼
      nvisy-engine  nvisy-detect  nvisy-python
             \      |      /
              ▼     ▼     ▼
              nvisy-core
                   ▲
                   |
             nvisy-object
```

Every crate depends on `nvisy-core`. Plugin crates (`nvisy-detect`, `nvisy-object`, `nvisy-python`) are independent of each other. The server imports everything and wires plugins into the engine at startup.

---

## 3. Core (`nvisy-core`)

### 3.1 Type system

All data flowing through a graph is represented as a `DataValue` — a discriminated union of typed primitives: `Document`, `Blob`, `Entity`, `Redaction`, `Policy`, `Audit`, `Image`. Each carries a `DataItem` with UUID, parent lineage, and metadata.

### 3.2 Traits

Extension points are defined as async traits:
- **Action** — transforms data (detect, redact, classify, emit audit)
- **Loader** — parses blobs into documents (plaintext, CSV, JSON)
- **ProviderFactory** — creates authenticated client connections
- **StreamSource / StreamTarget** — reads from / writes to external systems

### 3.3 Plugin registry

`PluginDescriptor` bundles actions, providers, sources, targets, and loaders under a namespace. `Registry` stores them keyed by `"plugin_id/item_id"` and resolves references at graph compilation time.

### 3.4 Errors

`Error` struct with `ErrorKind` enum (Validation, Connection, Timeout, Cancellation, Policy, Runtime, Python, Other). Carries optional source component, retryable flag, and boxed source error. `Result<T>` type alias for convenience.

---

## 4. Detection (`nvisy-detect`)

### 4.1 Pattern detection

Regex patterns are loaded from `assets/patterns.json` at startup. Each pattern defines: name, category, entity type, regex, confidence score, and optional validator reference. Validators (SSN format check, Luhn checksum) are registered in Rust code and resolved by name.

### 4.2 Actions

- **detect-regex** — scans documents against all or selected patterns, emits entities
- **detect-checksum** — validates entities with checksum algorithms (Luhn), boosts confidence
- **evaluate-policy** — filters entities against policy rules
- **apply-redaction** — applies redaction methods (mask, replace, hash, etc.)
- **classify** — categorizes documents based on detected entities
- **emit-audit** — produces audit records for compliance

### 4.3 Loaders

- **plaintext** — loads text files
- **csv** — loads CSV with header detection
- **json** — loads JSON documents

---

## 5. Engine (`nvisy-engine`)

### 5.1 Graph schema

Graphs are JSON structures with typed nodes (Source, Action, Target) and edges. Each node declares its provider/action reference, parameters, and optional retry/timeout policies.

### 5.2 Compilation

The compiler validates graph structure: parses JSON against the schema, checks for cycles via topological sort, verifies all node references resolve against the registry, and validates type compatibility between connected nodes.

### 5.3 Execution

The executor runs nodes in topological order. Data flows between nodes via `tokio::sync::mpsc` channels. Each node runs as a spawned task. The executor tracks per-node progress and aggregates results into a `RunResult`.

### 5.4 Run management

`RunManager` tracks all in-flight runs with status (pending, running, success, partial failure, failure, cancelled), progress per node, and cancellation tokens.

### 5.5 Policies

Retry policies (fixed, exponential, jitter backoff) and timeout policies are configurable per node.

---

## 6. Server (`nvisy-server`)

### 6.1 Role

Short-lived Axum HTTP server. Accepts graph definitions, compiles and executes them, reports status. Designed for containerized deployment.

### 6.2 REST API

| Method   | Path                        | Description                          |
|----------|-----------------------------|--------------------------------------|
| `GET`    | `/health`                   | Liveness probe                       |
| `GET`    | `/ready`                    | Readiness probe                      |
| `POST`   | `/api/v1/graphs/execute`    | Submit graph for execution           |
| `POST`   | `/api/v1/graphs/validate`   | Validate graph without executing     |
| `GET`    | `/api/v1/graphs`            | List runs                            |
| `GET`    | `/api/v1/graphs/{runId}`    | Get run status                       |
| `DELETE` | `/api/v1/graphs/{runId}`    | Cancel run                           |
| `POST`   | `/api/v1/redact`            | Submit redaction request             |
| `POST`   | `/api/v1/policies`          | Create policy                        |
| `GET`    | `/api/v1/policies`          | List policies                        |
| `GET`    | `/api/v1/policies/{id}`     | Get policy                           |
| `PUT`    | `/api/v1/policies/{id}`     | Update policy                        |
| `DELETE` | `/api/v1/policies/{id}`     | Delete policy                        |
| `GET`    | `/api/v1/audit`             | Query audit records                  |
| `GET`    | `/api/v1/audit/{runId}`     | Get audit records for a run          |

### 6.3 Middleware

- Request ID injection (`X-Request-Id`)
- Request/response tracing via `tower-http`
- CORS

### 6.4 Service layer

- `PolicyStore` — in-memory policy CRUD
- `AuditStore` — in-memory audit record storage
- `AppState` — shared state (registry, run manager, stores)
- `ServerConfig` — configuration from environment variables

---

## 7. Python Extensions

### 7.1 PyO3 bridge

`PythonBridge` manages Python interpreter access via `pyo3`. Functions run on `spawn_blocking` threads to avoid blocking the async runtime. The GIL is acquired per-call.

### 7.2 AI detection

The `nvisy-ai` Python package provides LLM-based NER for text and images. Called from Rust via the bridge, it returns entity dicts that are parsed into `Entity` structs.

### 7.3 EXIF handling

The `nvisy-exif` Python package reads and strips EXIF metadata from images using Pillow.

---

## 8. Error Handling

Errors carry an `ErrorKind`, message, optional source component, retryable flag, and optional boxed source error. The runtime distinguishes transient failures (retry with backoff) from terminal failures (fail immediately). Downstream nodes dependent on a failed node are skipped.

---

## 9. Security

- Credentials resolved from environment variables, never stored in graph definitions
- TLS termination and CORS via middleware
- Detection patterns configurable per deployment
- Audit trail for all detection and redaction operations
