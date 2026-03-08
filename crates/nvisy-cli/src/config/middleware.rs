//! HTTP middleware configuration.
//!
//! CLI flags override TOML `[server.middleware]` values, which override defaults.
//!
//! # Environment Variables
//!
//! - `BODY_LIMIT_MB` — Max body size in MiB for axum extractors (default: 4)
//! - `REQUEST_TIMEOUT_SECS` — Per-request timeout (default: 300s)
//! - `CORS_ALLOWED_ORIGINS` — Allowed CORS origins, comma-separated

use std::time::Duration;

use clap::Args;
use nvisy_server::middleware::{
    DEFAULT_MAX_BODY_SIZE, DEFAULT_MAX_FILE_BODY_SIZE, DEFAULT_REQUEST_TIMEOUT_SECS, OpenApiConfig,
    RecoveryConfig, SecurityConfig,
};

use super::file::MiddlewareSection;

const MB: usize = 1024 * 1024;

/// Middleware configuration for body limits, timeouts, and CORS.
///
/// All fields are optional — CLI flags override TOML, which overrides defaults.
#[derive(Debug, Clone, Args)]
pub struct MiddlewareConfig {
    /// Maximum body size in MiB for axum extractors (Json, Form, etc.).
    #[arg(long, env = "BODY_LIMIT_MB")]
    pub body_limit_mb: Option<usize>,

    /// Per-request timeout in seconds.
    #[arg(long, env = "REQUEST_TIMEOUT_SECS")]
    pub request_timeout_secs: Option<u64>,

    /// Allowed CORS origins (repeat for multiple). Empty means permissive.
    #[arg(long, env = "CORS_ALLOWED_ORIGINS", value_delimiter = ',')]
    pub cors_allowed_origins: Option<Vec<String>>,
}

impl MiddlewareConfig {
    /// Builds a [`SecurityConfig`], resolving CLI → TOML → defaults.
    pub fn security_config(&self, toml: &Option<MiddlewareSection>) -> SecurityConfig {
        let toml = toml.as_ref();

        let body_limit_mb = self
            .body_limit_mb
            .or_else(|| toml.and_then(|m| m.body_limit_mb));
        let body_limit_bytes = body_limit_mb
            .map(|mb| mb * MB)
            .unwrap_or(DEFAULT_MAX_BODY_SIZE);

        // File body limit stays at its default (not TOML-configurable).
        let file_body_limit_bytes = DEFAULT_MAX_FILE_BODY_SIZE;

        let cors_allowed_origins = self
            .cors_allowed_origins
            .clone()
            .or_else(|| {
                toml.and_then(|m| m.cors.as_ref())
                    .map(|c| c.allowed_origins.clone())
            })
            .unwrap_or_default();

        let cors_max_age_secs = toml.and_then(|m| m.cors.as_ref()).and_then(|c| c.max_age_secs);

        SecurityConfig {
            body_limit_bytes,
            file_body_limit_bytes,
            cors_allowed_origins,
            cors_max_age_secs,
        }
    }

    /// Builds a [`RecoveryConfig`], resolving CLI → TOML → defaults.
    pub fn recovery_config(&self, toml: &Option<MiddlewareSection>) -> RecoveryConfig {
        let toml = toml.as_ref();

        let timeout_secs = self
            .request_timeout_secs
            .or_else(|| toml.and_then(|m| m.request_timeout_secs))
            .unwrap_or(DEFAULT_REQUEST_TIMEOUT_SECS);

        RecoveryConfig {
            request_timeout: Duration::from_secs(timeout_secs),
        }
    }

    /// Returns the default [`OpenApiConfig`].
    pub fn open_api_config(&self) -> OpenApiConfig {
        OpenApiConfig::default()
    }
}
