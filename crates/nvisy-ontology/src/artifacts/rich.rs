//! Rich-document artifacts (text + image combined).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::image::ImageArtifacts;
use super::text::TextArtifacts;

/// Artifacts produced during processing of rich documents (PDF, DOCX).
///
/// Rich documents contain both text and image content, so their
/// artifacts compose both modality-specific artifact types.
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RichArtifacts {
    /// Text-modality artifacts.
    pub text: TextArtifacts,
    /// Image-modality artifacts (embedded images, OCR results).
    pub image: ImageArtifacts,
}
