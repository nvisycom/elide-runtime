//! File response types.

use nvisy_engine::{ContentDescriptor, ContentDigest};
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use super::page::Page;

/// Response body for `GET /files`.
pub type FileList = Page<FileMetadata>;

/// Metadata for a stored file. Returned by `GET /files/{id}` and
/// per-item by `GET /files`.
///
/// Annotations live at the separate `GET /files/{id}/annotations`
/// subresource and are not embedded here — bundling them would
/// couple the cacheability of the immutable descriptor/digest to
/// every annotation edit.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileMetadata {
    /// Identifier of the file.
    pub id: Uuid,
    /// Caller-supplied descriptor (filename, MIME hint, policy
    /// metadata).
    pub descriptor: ContentDescriptor,
    /// Byte-derived digest (size, sha256, sniffed MIME).
    pub digest: ContentDigest,
}

/// Response body for `POST /files`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileId {
    /// Identifier assigned to the uploaded file.
    pub id: Uuid,
}
