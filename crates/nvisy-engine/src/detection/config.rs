//! [`DetectionConfig`]: deployment-time `[detection.*]` configuration
//! the engine builds a [`DetectionEngine`] from at startup.
//!
//! [`DetectionEngine`]: super::DetectionEngine

use super::{LlmDetection, NerDetection, VlmDetection};

/// Configuration for the [`DetectionEngine`] registry. Each field
/// maps to a `[detection.*]` section in `Nvisy.toml`. Every field
/// is `Option<_>` so missing sections are valid — `None` means the
/// recognizer is not loaded.
///
/// Pattern detection is no longer a built-in: the consumer wires a
/// [`nvisy_pattern::PatternRecognizer`] (or any custom recognizer)
/// directly via [`DetectionEngine::add_text_recognizer`].
///
/// [`DetectionEngine`]: super::DetectionEngine
/// [`DetectionEngine::add_text_recognizer`]: super::DetectionEngine::add_text_recognizer
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct DetectionConfig {
    /// `[detection.llm]` — LLM-backed text recognizer config bundle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm: Option<LlmDetection>,
    /// `[detection.ner]` — NER recognizer config bundle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ner: Option<NerDetection>,
    /// `[detection.vlm]` — VLM-backed image recognizer config bundle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vlm: Option<VlmDetection>,
}
