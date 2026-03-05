//! HTTP middleware configuration.
//!
//! # Environment Variables
//!
//! - `BODY_LIMIT_BYTES` — Max body size for axum extractors (default: 2 MiB)
//! - `FILE_BODY_LIMIT_BYTES` — Max body size for file uploads (default: 50 MiB)
//! - `REQUEST_TIMEOUT_SECS` — Per-request timeout (default: 300s)
//! - `CORS_ALLOWED_ORIGINS` — Allowed CORS origins, comma-separated

use std::time::Duration;

use clap::Args;
use nvisy_server::middleware::{OpenApiConfig, RecoveryConfig, SecurityConfig};

/// Middleware configuration for body limits, timeouts, and CORS.
#[derive(Debug, Clone, Args)]
pub struct MiddlewareConfig {
    /// Maximum body size in bytes for axum extractors (Json, Form, etc.).
    #[arg(long, env = "BODY_LIMIT_BYTES", default_value_t = 2 * 1024 * 1024)]
    pub body_limit_bytes: usize,

    /// Maximum body size in bytes for file uploads.
    #[arg(long, env = "FILE_BODY_LIMIT_BYTES", default_value_t = 50 * 1024 * 1024)]
    pub file_body_limit_bytes: usize,

    /// Per-request timeout in seconds.
    #[arg(long, env = "REQUEST_TIMEOUT_SECS", default_value_t = 300)]
    pub request_timeout_secs: u64,

    /// Allowed CORS origins (repeat for multiple). Empty means permissive.
    #[arg(long, env = "CORS_ALLOWED_ORIGINS", value_delimiter = ',')]
    pub cors_allowed_origins: Vec<String>,
}

impl MiddlewareConfig {
    /// Builds a [`SecurityConfig`].
    pub fn security_config(&self) -> SecurityConfig {
        SecurityConfig {
            body_limit_bytes: self.body_limit_bytes,
            file_body_limit_bytes: self.file_body_limit_bytes,
            cors_allowed_origins: self.cors_allowed_origins.clone(),
        }
    }

    /// Builds a [`RecoveryConfig`].
    pub fn recovery_config(&self) -> RecoveryConfig {
        RecoveryConfig {
            request_timeout: Duration::from_secs(self.request_timeout_secs),
        }
    }

    /// Returns the default [`OpenApiConfig`].
    pub fn open_api_config(&self) -> OpenApiConfig {
        OpenApiConfig::default()
    }
}
