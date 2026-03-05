//! Context response types.

use nvisy_ontology::context::Context as OntologyContext;
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

/// Response body for `POST /api/v1/contexts`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContextId {
    /// Identifier assigned to the uploaded context.
    pub id: Uuid,
}

/// Response body for `GET /api/v1/contexts/{id}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    /// Identifier of the context.
    pub id: Uuid,
    /// The stored context.
    pub context: OntologyContext,
}

/// Response body for `GET /api/v1/contexts`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContextList {
    /// List of context identifiers.
    pub contexts: Vec<Uuid>,
}
