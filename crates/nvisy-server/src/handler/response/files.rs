//! File response types.

use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use super::page::Page;

/// Response body for `GET /files`.
pub type FileList = Page<FileEntry>;

/// Metadata for a stored file. Returned both inline by
/// `GET /files/{id}` and as list entries by `GET /files`.
///
/// File bytes themselves are served separately by
/// `GET /files/{id}/content` (octet-stream) so the JSON metadata
/// shape stays small regardless of file size.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    /// Identifier of the file.
    pub id: Uuid,
    /// Original filename, if provided at upload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    /// MIME type (supplied or detected).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// Content size in bytes. Backfilled at registration time.
    pub size: u64,
    /// SHA-256 hex digest. Backfilled at registration time.
    pub sha256: String,
}

/// Response body for `POST /files`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileId {
    /// Identifier assigned to the uploaded file.
    pub id: Uuid,
}
