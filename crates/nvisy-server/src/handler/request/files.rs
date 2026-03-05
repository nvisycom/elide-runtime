//! File request types.

use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use super::Base64;

/// JSON request body for base64-encoded file upload.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewFile {
    /// Actor identity that owns the file.
    pub actor_id: Uuid,
    /// Base64-encoded file bytes.
    pub content: Base64,
    /// Optional original filename.
    #[serde(default)]
    pub filename: Option<String>,
    /// Optional MIME type override (e.g. `text/csv`).
    #[serde(default)]
    pub content_type: Option<String>,
}
