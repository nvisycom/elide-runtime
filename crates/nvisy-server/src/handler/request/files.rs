//! File request types.

use nvisy_document::document::AnyAnnotations;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::handler::utility::Base64;

/// Request body for `POST /files`: base64-encoded file upload with
/// optional uploader-supplied annotations.
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
    /// Uploader-supplied per-modality annotation buckets and
    /// document-level labels. `Hint` inclusions are folded into
    /// the LLM detect prompt for per-hint adjudication; `Assert`
    /// inclusions become synthetic entities at import; exclusions
    /// drop matching detections post-filter; labels propagate to
    /// every modality envelope spawned from this upload.
    #[serde(default)]
    pub annotations: AnyAnnotations,
}
