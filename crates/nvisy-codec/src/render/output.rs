//! Unified redaction output enum combining all modality-specific outputs.

use derive_more::From;
use serde::{Deserialize, Serialize};

use crate::render::audio::AudioRedactionOutput;
use crate::render::image::ImageRedactionOutput;
use crate::render::text::TextRedactionOutput;

/// Unified redaction output that wraps modality-specific output variants.
///
/// Carries method-specific result data (replacement strings, ciphertext,
/// blur sigma, etc.).
#[derive(Debug, Clone, PartialEq, From, Serialize, Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RedactionOutput {
    /// Text/tabular redaction output.
    Text(TextRedactionOutput),
    /// Image/video redaction output.
    Image(ImageRedactionOutput),
    /// Audio redaction output.
    Audio(AudioRedactionOutput),
}
