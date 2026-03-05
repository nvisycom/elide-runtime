//! Typed path and query parameters for API endpoints.

use nvisy_registry::{ActorId, ContentId, ContextId};
use schemars::JsonSchema;
use serde::Deserialize;

/// Path parameter for file endpoints.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ContentPath {
    /// Content identifier.
    pub id: ContentId,
}

/// Path parameter for context endpoints.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ContextPath {
    /// Context identifier.
    pub id: ContextId,
}

/// Query parameter for endpoints that need actor scoping.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActorQuery {
    /// Actor identity.
    pub actor_id: ActorId,
}
