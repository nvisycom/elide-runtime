//! File response types.

use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use crate::handler::utility::Base64;

/// Response body for `POST /api/v1/files`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileId {
    /// Identifier assigned to the uploaded file.
    pub id: Uuid,
}

/// Response body for `GET /api/v1/files/{id}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct File {
    /// Identifier of the file.
    pub id: Uuid,
    /// Base64-encoded file bytes.
    pub content: Base64,
    /// MIME type of the file, if provided at upload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// Original filename, if provided at upload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
}

/// Response body for `GET /api/v1/files`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileList {
    /// List of file identifiers.
    pub files: Vec<Uuid>,
}
