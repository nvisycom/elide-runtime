//! Router construction with middleware composition.
//!
//! Assembles the full [`axum::Router`] from handler routes and the middleware
//! stack. The middleware is applied in this order (outermost first):
//!
//! 1. **OpenAPI specification** — finalises the aide route tree, serves spec
//!    JSON and Scalar UI.
//! 2. **Recovery** — catches panics and enforces per-request timeouts.
//! 3. **Observability** — request IDs, structured tracing, sensitive header
//!    redaction.
//! 4. **Security** — CORS policy and request body size limits.
//! 5. **Compression** — gzip, brotli, and zstd response compression.

use crate::handler;
use crate::middleware::compression::RouterCompressionExt;
use crate::middleware::observability::RouterObservabilityExt;
use crate::middleware::recovery::RouterRecoveryExt;
use crate::middleware::security::RouterSecurityExt;
use crate::middleware::specification::RouterOpenApiExt;
use crate::service::ServiceState;

use super::config::ServerConfig;

/// Builds the application router with all middleware layers applied.
pub fn build(config: &ServerConfig, state: ServiceState) -> axum::Router {
    handler::routes()
        .with_open_api(&config.open_api_config())
        .with_recovery(&config.recovery_config())
        .with_observability()
        .with_security(&config.security_config())
        .with_compression()
        .with_state(state)
}
