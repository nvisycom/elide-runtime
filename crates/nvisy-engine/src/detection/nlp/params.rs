//! [`NlpDetection`]: NER-specific knobs in the workflow detection
//! config.
//!
//! Cross-recognizer hints (`entity_kinds`, `confidence_threshold`)
//! live directly on [`Detection`] because every recognizer honors
//! them. This struct carries the bits that only mean something for
//! the ONNX NER path — today, the prebuilt engine preset.
//!
//! [`Detection`]: crate::detection::Detection

use nvisy_nlp::preset::NlpPreset;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// NER-specific detection settings.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct NlpDetection {
    /// Enable this recognizer. When `false`, the recognizer is
    /// neither built nor dispatched, but the config is preserved
    /// so operators can toggle without losing it. Defaults to
    /// `true`.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Prebuilt NLP-engine preset to load. [`NlpPreset::Default`]
    /// for the placeholder no-op backend until real model bundles
    /// land.
    #[serde(default)]
    pub engine: NlpPreset,
}

impl Default for NlpDetection {
    fn default() -> Self {
        Self {
            enabled: true,
            engine: NlpPreset::default(),
        }
    }
}

fn default_true() -> bool {
    true
}
