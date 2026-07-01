//! HTTP middleware configuration.
//!
//! Data types plus resolution methods that map into the
//! runtime middleware config types.

use std::time::Duration;

use crate::middleware::{
    DEFAULT_MAX_BODY_SIZE, DEFAULT_MAX_FILE_BODY_SIZE, DEFAULT_REQUEST_TIMEOUT, RecoveryConfig,
    SecurityConfig,
};
use serde::Deserialize;

const MB: usize = 1024 * 1024;

/// `[server.middleware]`: body limits, request timeout, and CORS.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MiddlewareConfig {
    /// Maximum request body size in MiB for axum extractors. Default: 4.
    pub body_limit_mb: Option<usize>,
    /// Per-request timeout. Parses human-readable durations
    /// (`"5m"`, `"300s"`). Default: 5m.
    #[serde(default, with = "humantime_serde")]
    pub request_timeout: Option<Duration>,
    /// CORS policy. Omit for permissive defaults.
    pub cors: Option<CorsConfig>,
}

/// `[server.middleware.cors]`: CORS policy.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorsConfig {
    /// Origins allowed to make cross-origin requests.
    /// An empty list (or omitted) means permissive (all origins).
    #[serde(default)]
    pub allowed_origins: Vec<String>,
    /// `Access-Control-Max-Age` for preflight responses. Parses
    /// human-readable durations (`"1h"`, `"3600s"`).
    #[serde(default, with = "humantime_serde")]
    pub max_age: Option<Duration>,
}

impl MiddlewareConfig {
    /// Build a [`SecurityConfig`] from this section.
    #[must_use]
    pub fn security(&self) -> SecurityConfig {
        let body_limit_bytes = self
            .body_limit_mb
            .map(|mb| mb * MB)
            .unwrap_or(DEFAULT_MAX_BODY_SIZE);
        let cors_allowed_origins = self
            .cors
            .as_ref()
            .map(|c| c.allowed_origins.clone())
            .unwrap_or_default();
        let cors_max_age = self.cors.as_ref().and_then(|c| c.max_age);
        SecurityConfig {
            body_limit_bytes,
            file_body_limit_bytes: DEFAULT_MAX_FILE_BODY_SIZE,
            cors_allowed_origins,
            cors_max_age,
        }
    }

    /// Build a [`RecoveryConfig`] from this section.
    #[must_use]
    pub fn recovery(&self) -> RecoveryConfig {
        let request_timeout = self.request_timeout.unwrap_or(DEFAULT_REQUEST_TIMEOUT);
        RecoveryConfig { request_timeout }
    }
}
