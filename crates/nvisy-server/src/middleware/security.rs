//! Security middleware for HTTP request protection.
//!
//! Provides CORS policy, request body size limiting (both the axum
//! [`DefaultBodyLimit`] and tower-http [`RequestBodyLimitLayer`]), and
//! response compression. The two body limits serve different purposes:
//!
//! - [`DefaultBodyLimit`]: governs axum extractors (`Json`, `Form`, etc.).
//! - [`RequestBodyLimitLayer`]: hard cap enforced by tower-http on the raw
//!   request body, including multipart file uploads.
//!
//! [`DefaultBodyLimit`]: axum::extract::DefaultBodyLimit
//! [`RequestBodyLimitLayer`]: tower_http::limit::RequestBodyLimitLayer

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::HeaderValue;
use serde::{Deserialize, Serialize};
use tower_http::compression::CompressionLayer;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;

/// Default request body limit for axum extractors (2 MiB).
const DEFAULT_BODY_LIMIT: usize = 2 * 1024 * 1024;

/// Default request body limit for file uploads (50 MiB).
const DEFAULT_FILE_BODY_LIMIT: usize = 50 * 1024 * 1024;

/// Configuration for security middleware.
///
/// Controls CORS policy and request body size limits. The two limit
/// fields target different layers of the stack: see the module-level
/// documentation for details.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityConfig {
    /// Maximum body size in bytes for axum extractors (`Json`, `Form`, etc.).
    pub body_limit_bytes: usize,

    /// Maximum body size in bytes for the raw request body (file uploads).
    pub file_body_limit_bytes: usize,

    /// Allowed CORS origins. An empty list permits all origins (permissive).
    #[serde(default)]
    pub cors_allowed_origins: Vec<String>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            body_limit_bytes: DEFAULT_BODY_LIMIT,
            file_body_limit_bytes: DEFAULT_FILE_BODY_LIMIT,
            cors_allowed_origins: Vec::new(),
        }
    }
}

/// Extension trait for [`Router`] to apply security middleware.
///
/// Layers CORS, body limits, and response compression in a single call.
pub trait RouterSecurityExt<S> {
    /// Layers security middleware with the provided configuration.
    fn with_security(self, config: &SecurityConfig) -> Self;
}

impl<S> RouterSecurityExt<S> for Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn with_security(self, config: &SecurityConfig) -> Self {
        let cors = if config.cors_allowed_origins.is_empty() {
            CorsLayer::permissive()
        } else {
            let origins: Vec<HeaderValue> = config
                .cors_allowed_origins
                .iter()
                .filter_map(|o| o.parse().ok())
                .collect();
            CorsLayer::new().allow_origin(AllowOrigin::list(origins))
        };

        self.layer(DefaultBodyLimit::max(config.body_limit_bytes))
            .layer(RequestBodyLimitLayer::new(config.file_body_limit_bytes))
            .layer(CompressionLayer::new())
            .layer(cors)
    }
}
