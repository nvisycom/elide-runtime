//! Check response types (health, analytics).

use std::borrow::Cow;

use jiff::Timestamp;
use schemars::JsonSchema;
use serde::Serialize;

/// Represents the operational status of a service.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ServiceStatus {
    /// Service is operating normally.
    #[default]
    Healthy,
    /// Service is operating with some issues but still functional.
    Degraded,
    /// Service is not operational.
    Unhealthy,
}

/// Health status of a single service component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ComponentCheck {
    /// Component name (e.g. `"filesystem"`, `"registry"`).
    pub name: Cow<'static, str>,
    /// Status of this component.
    pub status: ServiceStatus,
}

/// Response body for `GET /health`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    /// Overall service status.
    pub status: ServiceStatus,
    /// Per-component health checks.
    pub checks: Vec<ComponentCheck>,
    /// RFC 3339 timestamp of when the check was performed.
    #[schemars(with = "String")]
    pub timestamp: Timestamp,
}
