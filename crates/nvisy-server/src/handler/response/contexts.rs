//! Context response types.

use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use crate::handler::request::Page;

/// Response body for `GET /contexts`.
pub type ContextList = Page<ContextEntry>;

/// Summary of a stored context for listing endpoints.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContextEntry {
    /// Identifier of the context.
    pub id: Uuid,
    /// Human-readable label.
    pub name: String,
    /// Number of reference-data entries in this context.
    pub entries: usize,
}

/// Response body for `POST /contexts`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContextId {
    /// Identifier assigned to the uploaded context.
    pub id: Uuid,
}
