//! Health response body.

use jiff::Timestamp;
use nvisy_core::service::health::{ComponentCheck, ServiceStatus};
use schemars::JsonSchema;
use serde::Serialize;

/// Response body for `GET /health`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    /// Overall service status.
    pub status: ServiceStatus,
    /// Per-component checks.
    pub checks: Vec<ComponentCheck>,
    /// RFC 3339 timestamp of when the check was performed.
    #[schemars(with = "String")]
    pub timestamp: Timestamp,
}
