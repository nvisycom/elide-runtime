//! Policy response types.

use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use crate::handler::request::Page;

/// Response body for `GET /policies`.
pub type PolicyList = Page<PolicyEntry>;

/// Summary of a stored policy for listing endpoints.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PolicyEntry {
    /// Identifier of the policy.
    pub id: Uuid,
    /// Human-readable policy name.
    pub name: String,
    /// Policy version string.
    pub version: String,
}

/// Response body for `POST /policies`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PolicyId {
    /// Identifier assigned to the uploaded policy.
    pub id: Uuid,
}
