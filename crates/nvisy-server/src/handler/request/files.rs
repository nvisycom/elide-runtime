//! File request types.

use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

/// JSON request body for base64-encoded file upload.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileUpload {
    /// Base64-encoded file bytes.
    pub content: String,
    /// Optional original filename.
    #[serde(default)]
    pub filename: Option<String>,
    /// Optional MIME type override (e.g. `text/csv`).
    #[serde(default)]
    pub content_type: Option<String>,
    /// Optional actor identity. Defaults to a nil UUID when absent.
    #[serde(default)]
    pub actor_id: Option<Uuid>,
}
