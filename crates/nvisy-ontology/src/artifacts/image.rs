//! Image-modality artifacts.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Text extracted from an image region via OCR or multimodal model.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OcrResult {
    /// The extracted text.
    pub text: String,
    /// Confidence score of the extraction, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

/// Artifacts produced during processing of image content.
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImageArtifacts {
    /// OCR-extracted text regions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ocr_results: Vec<OcrResult>,
}
