//! [`NlpDetection`]: NER-specific knobs in the workflow detection
//! config.
//!
//! Cross-recognizer hints (`entity_kinds`, `confidence_threshold`)
//! live on [`DetectionParams`] because every recognizer honors
//! them. This struct carries the bits that only mean something for
//! the ONNX NER path — today, the prebuilt engine preset.
//!
//! [`DetectionParams`]: crate::recognizer::DetectionParams

use nvisy_nlp::NlpPreset;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// NER-specific detection settings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct NlpDetection {
    /// Prebuilt NLP-engine preset to load. [`NlpPreset::Default`]
    /// for the placeholder no-op backend until real model bundles
    /// land.
    #[serde(default)]
    pub engine: NlpPreset,
}
