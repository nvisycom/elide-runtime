//! Context response types.

use nvisy_ontology::context::Context;
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

/// Response body for `POST /api/v1/contexts`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContextUploadResponse {
    /// Identifier assigned to the uploaded context.
    pub id: Uuid,
}

/// Response body for `GET /api/v1/contexts/{id}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContextDownloadResponse {
    /// Identifier of the context.
    pub id: Uuid,
    /// The stored context.
    pub context: Context,
}

/// Response body for `DELETE /api/v1/contexts/{id}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContextDeleteResponse {
    /// Identifier of the deleted context.
    pub id: Uuid,
}

/// Response body for `GET /api/v1/contexts`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContextListResponse {
    /// List of context identifiers.
    pub contexts: Vec<Uuid>,
}

/// Response body for `DELETE /api/v1/contexts`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContextDeleteAllResponse {
    /// Number of contexts deleted.
    pub deleted: usize,
}
