//! Context request types.

use schemars::JsonSchema;
use serde::Deserialize;

/// JSON request body for base64-encoded context upload.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContextUpload {
    /// Base64-encoded context bytes.
    pub content: String,
    /// Optional original filename.
    #[serde(default)]
    pub filename: Option<String>,
    /// Optional MIME type override (e.g. `text/csv`).
    #[serde(default)]
    pub content_type: Option<String>,
}
