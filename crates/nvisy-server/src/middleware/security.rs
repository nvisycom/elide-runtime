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

use std::time::Duration;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::HeaderValue;
use serde::{Deserialize, Serialize};
use tower_http::compression::CompressionLayer;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;

use super::constants::{DEFAULT_MAX_BODY_SIZE, DEFAULT_MAX_FILE_BODY_SIZE};

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

    /// Optional max-age for CORS preflight responses.
    pub cors_max_age: Option<Duration>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            body_limit_bytes: DEFAULT_MAX_BODY_SIZE,
            file_body_limit_bytes: DEFAULT_MAX_FILE_BODY_SIZE,
            cors_allowed_origins: Vec::new(),
            cors_max_age: None,
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
        let origins = &config.cors_allowed_origins;
        let cors = if origins.is_empty() || origins.iter().any(|o| o == "*") {
            // Empty list = no policy = permissive; explicit "*" means
            // allow any origin. tower_http rejects "*" inside
            // `AllowOrigin::list`, so route it through `any()` here.
            CorsLayer::new().allow_origin(AllowOrigin::any())
        } else {
            let parsed: Vec<HeaderValue> = origins.iter().filter_map(|o| o.parse().ok()).collect();
            CorsLayer::new().allow_origin(AllowOrigin::list(parsed))
        };

        let cors = if let Some(max_age) = config.cors_max_age {
            cors.max_age(max_age)
        } else {
            cors
        };

        self.layer(DefaultBodyLimit::max(config.body_limit_bytes))
            .layer(RequestBodyLimitLayer::new(config.file_body_limit_bytes))
            .layer(CompressionLayer::new())
            .layer(cors)
    }
}
