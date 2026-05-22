//! Middleware resolution from TOML `[server.middleware]`.
//!
//! Translates the optional [`MiddlewareSection`] from the TOML file into
//! concrete middleware configs consumed by `nvisy-server`. When a TOML
//! field is absent, the corresponding `nvisy-server` default is used.

use std::time::Duration;

use nvisy_server::middleware::{
    DEFAULT_MAX_BODY_SIZE, DEFAULT_MAX_FILE_BODY_SIZE, DEFAULT_REQUEST_TIMEOUT, OpenApiConfig,
    RecoveryConfig, SecurityConfig,
};

use super::file::MiddlewareSection;

const MB: usize = 1024 * 1024;

/// Builds a [`SecurityConfig`] from the TOML middleware section.
///
/// Resolves body limits, CORS origins, and CORS max-age. Falls back to
/// `nvisy-server` defaults for any omitted field.
pub fn security_config(toml: &Option<MiddlewareSection>) -> SecurityConfig {
    let toml = toml.as_ref();

    let body_limit_bytes = toml
        .and_then(|m| m.body_limit_mb)
        .map(|mb| mb * MB)
        .unwrap_or(DEFAULT_MAX_BODY_SIZE);

    let cors_allowed_origins = toml
        .and_then(|m| m.cors.as_ref())
        .map(|c| c.allowed_origins.clone())
        .unwrap_or_default();

    let cors_max_age = toml.and_then(|m| m.cors.as_ref()).and_then(|c| c.max_age);

    SecurityConfig {
        body_limit_bytes,
        file_body_limit_bytes: DEFAULT_MAX_FILE_BODY_SIZE,
        cors_allowed_origins,
        cors_max_age,
    }
}

/// Builds a [`RecoveryConfig`] from the TOML middleware section.
///
/// Uses the configured `request_timeout` or falls back to the
/// `nvisy-server` default (5 min).
pub fn recovery_config(toml: &Option<MiddlewareSection>) -> RecoveryConfig {
    let request_timeout: Duration = toml
        .as_ref()
        .and_then(|m| m.request_timeout)
        .unwrap_or(DEFAULT_REQUEST_TIMEOUT);

    RecoveryConfig { request_timeout }
}

/// Returns the default [`OpenApiConfig`].
pub fn open_api_config() -> OpenApiConfig {
    OpenApiConfig::default()
}
