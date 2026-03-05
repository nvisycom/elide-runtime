//! File response types.

use nvisy_registry::ContentId;
use schemars::JsonSchema;
use serde::Serialize;

use super::super::request::Base64;

/// Response body for `POST /api/v1/files`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileUploadResponse {
    /// Identifier assigned to the uploaded file.
    pub id: ContentId,
}

/// Response body for `GET /api/v1/files/{id}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileDownloadResponse {
    /// Identifier of the file.
    pub id: ContentId,
    /// Base64-encoded file bytes.
    pub content: Base64,
}

/// Response body for `DELETE /api/v1/files/{id}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileDeleteResponse {
    /// Identifier of the deleted file.
    pub id: ContentId,
}

/// Response body for `GET /api/v1/files`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileListResponse {
    /// List of file identifiers.
    pub files: Vec<ContentId>,
}

/// Response body for `DELETE /api/v1/files`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileDeleteAllResponse {
    /// Number of files deleted.
    pub deleted: usize,
}
