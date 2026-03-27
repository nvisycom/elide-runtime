//! File response types.

use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use crate::handler::request::Page;
use crate::handler::utility::Base64;

/// Response body for `GET /api/v1/files`.
pub type FileList = Page<FileSummary>;

/// Summary of a stored file for listing endpoints.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileSummary {
    /// Identifier of the file.
    pub id: Uuid,
    /// Original filename, if provided at upload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    /// MIME type (supplied or detected).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// Content size in bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// SHA-256 hex digest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

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
    /// MIME type (supplied or detected).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// Original filename, if provided at upload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    /// Content size in bytes.
    pub size: u64,
    /// SHA-256 hex digest.
    pub sha256: String,
}
