//! Typed path parameters for API endpoints.

use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

/// Path parameter for endpoints scoped to a single content item.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ContentPath {
    /// Content identifier.
    pub id: Uuid,
}
