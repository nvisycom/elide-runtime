//! Middleware resolution from TOML `[server.middleware]`.

use std::time::Duration;

use nvisy_server::middleware::{
    DEFAULT_MAX_BODY_SIZE, DEFAULT_MAX_FILE_BODY_SIZE, DEFAULT_REQUEST_TIMEOUT_SECS, OpenApiConfig,
    RecoveryConfig, SecurityConfig,
};

use super::file::MiddlewareSection;

const MB: usize = 1024 * 1024;

/// Resolve a [`SecurityConfig`] from `[server.middleware]` TOML → defaults.
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

    let cors_max_age_secs = toml
        .and_then(|m| m.cors.as_ref())
        .and_then(|c| c.max_age_secs);

    SecurityConfig {
        body_limit_bytes,
        file_body_limit_bytes: DEFAULT_MAX_FILE_BODY_SIZE,
        cors_allowed_origins,
        cors_max_age_secs,
    }
}

/// Resolve a [`RecoveryConfig`] from `[server.middleware]` TOML → defaults.
pub fn recovery_config(toml: &Option<MiddlewareSection>) -> RecoveryConfig {
    let timeout_secs = toml
        .as_ref()
        .and_then(|m| m.request_timeout_secs)
        .unwrap_or(DEFAULT_REQUEST_TIMEOUT_SECS);

    RecoveryConfig {
        request_timeout: Duration::from_secs(timeout_secs),
    }
}

/// Returns the default [`OpenApiConfig`].
pub fn open_api_config() -> OpenApiConfig {
    OpenApiConfig::default()
}
