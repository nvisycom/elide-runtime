//! HTTP middleware stack.
//!
//! Each submodule provides a configuration struct (where applicable) and an
//! extension trait on [`Router`](axum::Router) (or
//! [`ApiRouter`](aide::axum::ApiRouter) for OpenAPI). Middleware is composed
//! by chaining `.with_*()` calls in
//! [`server::router::build`](crate::server::router::build).
//!
//! | Module            | Trait                     | Purpose                                  |
//! |-------------------|---------------------------|------------------------------------------|
//! | [`specification`] | `RouterOpenApiExt`        | OpenAPI spec generation and Scalar UI    |
//! | [`recovery`]      | `RouterRecoveryExt`       | Panic catching and request timeouts      |
//! | [`observability`] | `RouterObservabilityExt`  | Request IDs, tracing, header redaction   |
//! | [`security`]      | `RouterSecurityExt`       | CORS policy and body size limits         |
//! | [`compression`]   | `RouterCompressionExt`    | gzip, brotli, and zstd compression       |

pub mod compression;
pub mod observability;
pub mod recovery;
pub mod security;
pub mod specification;
