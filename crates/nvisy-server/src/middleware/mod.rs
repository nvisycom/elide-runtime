//! Middleware for [`axum::Router`] and HTTP request processing.
//!
//! This module provides middleware for security, observability, error recovery,
//! response compression, and API documentation. Each middleware category has its
//! own extension trait for ergonomic composition.
//!
//! # Middleware Ordering
//!
//! The order in which middleware is applied matters significantly. Axum applies
//! layers in reverse order, meaning the last layer added wraps the outermost
//! request handling. The recommended ordering from outermost to innermost is:
//!
//! 1. **Recovery** - Catches panics and enforces timeouts at the outermost layer,
//!    ensuring all errors are properly handled regardless of where they occur.
//!
//! 2. **Observability** - Generates request IDs and adds tracing spans early,
//!    so all subsequent middleware and handlers are properly instrumented.
//!
//! 3. **Security** - Applies CORS policy, body size limits, and response
//!    compression before any request processing occurs.
//!
//! # Example
//!
//! ```rust,ignore
//! use axum::Router;
//! use nvisy_server::middleware::{
//!     OpenApiConfig, RouterOpenApiExt,
//!     RecoveryConfig, RouterRecoveryExt,
//!     RouterObservabilityExt,
//!     SecurityConfig, RouterSecurityExt,
//! };
//! use nvisy_server::ServiceState;
//!
//! fn create_router(state: ServiceState) -> Router {
//!     nvisy_server::routes()
//!         .with_open_api(&OpenApiConfig::default())     // ApiRouter<S> -> Router<S>
//!         .with_recovery(&RecoveryConfig::default())    // 1. Recovery (outermost)
//!         .with_observability()                         // 2. Observability
//!         .with_security(&SecurityConfig::default())    // 3. Security + compression
//!         .with_state(state)
//! }
//! ```

mod observability;
mod recovery;
mod security;
mod specification;

pub use observability::RouterObservabilityExt;
pub use recovery::{RecoveryConfig, RouterRecoveryExt};
pub use security::{SecurityConfig, RouterSecurityExt};
pub use specification::{OpenApiConfig, RouterOpenApiExt};
