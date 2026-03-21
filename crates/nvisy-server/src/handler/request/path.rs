//! Typed path parameters for API endpoints.

use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

/// Path parameter for file endpoints.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ContentPath {
    /// Content identifier.
    pub id: Uuid,
}

/// Path parameter for context endpoints.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ContextPath {
    /// Context identifier.
    pub id: Uuid,
}

/// Path parameter for run endpoints.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RunPath {
    /// Run identifier.
    pub id: Uuid,
}
