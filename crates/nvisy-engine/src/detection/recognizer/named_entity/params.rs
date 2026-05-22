//! [`NerDetection`]: NER-specific knobs in the workflow detection
//! config.
//!
//! Cross-recognizer hints (`entity_kinds`, `confidence_threshold`)
//! live on [`DetectionParams`] because every recognizer honors
//! them. This struct carries the bits that only mean something for
//! the ONNX NER path — today, the prebuilt engine preset.
//!
//! [`DetectionParams`]: crate::recognizer::DetectionParams

use nvisy_nlp::NerEngine;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// NER-specific detection settings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct NerDetection {
    /// Prebuilt NLP-engine preset to load. [`NerEngine::Default`]
    /// for the placeholder no-op backend until real model bundles
    /// land.
    #[serde(default)]
    pub engine: NerEngine,
}
