//! Typed path and query parameters for API endpoints.

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

/// Query parameter for endpoints that need actor scoping.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActorQuery {
    /// Actor identity.
    pub actor_id: Uuid,
}
