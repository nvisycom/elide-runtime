//! [`DetectionConfig`]: deployment-time `[detection.*]` configuration
//! the engine builds a [`Recognizers`] registry from at startup.
//!
//! [`Recognizers`]: super::Recognizers

use super::{LlmDetection, NerDetection, PatternDetection, VlmDetection};

/// Configuration for the [`Recognizers`] registry. Each field maps
/// to a `[detection.*]` section in `Nvisy.toml`. `None` opts the
/// recognizer out entirely.
///
/// [`Recognizers`]: super::Recognizers
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct DetectionConfig {
    /// `[detection.llm]` — LLM-backed text recognizer config bundle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm: Option<LlmDetection>,
    /// `[detection.ner]` — NER recognizer config bundle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ner: Option<NerDetection>,
    /// `[detection.pattern]` — pattern recognizer config bundle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<PatternDetection>,
    /// `[detection.vlm]` — VLM-backed image recognizer config bundle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vlm: Option<VlmDetection>,
}

impl DetectionConfig {
    /// `true` when every section is `None`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.llm.is_none() && self.ner.is_none() && self.pattern.is_none() && self.vlm.is_none()
    }
}
