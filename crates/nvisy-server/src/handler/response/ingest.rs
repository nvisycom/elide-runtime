//! Ingest response types.

use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

/// Response body for `POST /api/v1/ingest`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UploadResponse {
    /// Identifier assigned to the uploaded content.
    pub id: Uuid,
}

/// Response body for `GET /api/v1/ingest/{id}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DownloadResponse {
    /// Identifier of the content.
    pub id: Uuid,
    /// Base64-encoded content bytes.
    pub content: String,
}

/// Response body for `DELETE /api/v1/ingest/{id}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteResponse {
    /// Identifier of the deleted content.
    pub id: Uuid,
}

/// Response body for `DELETE /api/v1/ingest`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAllResponse {
    /// Number of content items deleted.
    pub deleted: usize,
}
