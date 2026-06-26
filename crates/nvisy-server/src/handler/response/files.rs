//! File response shapes.

use nvisy_core::FileMetadata;
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

/// Just-uploaded id; returned by `POST /files` so clients can
/// reference the file in subsequent calls without parsing the
/// metadata.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileId {
    /// Engine-minted UUIDv7.
    pub id: Uuid,
}

/// Re-export of [`FileMetadata`] as the response shape (already
/// has `Serialize + JsonSchema` via nvisy-core).
pub type FileMetadataResponse = FileMetadata;
