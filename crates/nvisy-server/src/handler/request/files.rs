//! File request types.

use schemars::JsonSchema;
use serde::Deserialize;

use crate::handler::utility::Base64;

/// Request body for `POST /files`: base64-encoded file upload.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewFile {
    /// Base64-encoded file bytes.
    pub content: Base64,
    /// Optional original filename.
    #[serde(default)]
    pub filename: Option<String>,
    /// Optional MIME type override (e.g. `text/csv`).
    #[serde(default)]
    pub content_type: Option<String>,
}
