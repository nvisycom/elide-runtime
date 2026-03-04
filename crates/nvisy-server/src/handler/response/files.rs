//! File response types.

use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

/// Response body for `POST /api/v1/files`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UploadResponse {
    /// Identifier assigned to the uploaded file.
    pub id: Uuid,
}

/// Response body for `GET /api/v1/files/{id}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DownloadResponse {
    /// Identifier of the file.
    pub id: Uuid,
    /// Base64-encoded file bytes.
    pub content: String,
}

/// Response body for `DELETE /api/v1/files/{id}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteResponse {
    /// Identifier of the deleted file.
    pub id: Uuid,
}

/// Response body for `DELETE /api/v1/files`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAllResponse {
    /// Number of files deleted.
    pub deleted: usize,
}
