//! Security middleware.
//!
//! Provides CORS policy and request body size limits.

use axum::Router;
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;

/// Default request body limit (50 MiB).
const DEFAULT_BODY_LIMIT: usize = 50 * 1024 * 1024;

/// Configuration for security middleware.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Maximum request body size in bytes.
    pub body_limit_bytes: usize,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            body_limit_bytes: DEFAULT_BODY_LIMIT,
        }
    }
}

/// Extension trait for [`Router`] to add security middleware.
pub trait RouterSecurityExt<S> {
    /// Layers CORS and body limit middleware.
    fn with_security(self, config: &SecurityConfig) -> Self;
}

impl<S> RouterSecurityExt<S> for Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn with_security(self, config: &SecurityConfig) -> Self {
        self.layer(CorsLayer::permissive())
            .layer(RequestBodyLimitLayer::new(config.body_limit_bytes))
    }
}
