//! Rich-document artifacts (text + image + tabular combined).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::image::ImageArtifacts;
use super::tabular::TabularArtifacts;
use super::text::TextArtifacts;

/// Artifacts produced during processing of rich documents (PDF, DOCX).
///
/// Rich documents can contain text, images, and tables, so their
/// artifacts compose all three modality-specific artifact types.
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RichArtifacts {
    /// Text-modality artifacts.
    pub text: TextArtifacts,
    /// Image-modality artifacts (embedded images, OCR results).
    pub image: ImageArtifacts,
    /// Tabular-modality artifacts (embedded tables).
    pub tabular: TabularArtifacts,
}
