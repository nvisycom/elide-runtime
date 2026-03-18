//! Extraction action configurations: visual and audial.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Configuration for the [`VisualExtraction`] action.
///
/// [`VisualExtraction`]: super::GraphNodeKind::VisualExtraction
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct VisualExtraction {
    /// Run a secondary LLM verification pass on OCR results.
    #[serde(default)]
    pub verification: bool,
    /// Run computer vision entity detection on images.
    #[serde(default)]
    pub entity_detection: bool,
}

/// Configuration for the [`AudialExtraction`] action.
///
/// [`AudialExtraction`]: super::GraphNodeKind::AudialExtraction
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct AudialExtraction {
    /// Segment the audio by speaker identity.
    #[serde(default)]
    pub diarization: bool,
}
