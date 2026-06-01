//! [`DetectionConfig`]: deployment-time `[detection.*]` configuration
//! the engine builds a [`DetectionEngine`] from at startup.
//!
//! [`DetectionEngine`]: super::DetectionEngine

use super::{LlmDetection, NerDetection, PatternDetection, VlmDetection};

/// Configuration for the [`DetectionEngine`] registry. Each field
/// maps to a `[detection.*]` section in `Nvisy.toml`. Every field
/// is `Option<_>` so missing sections are valid — what `None` means
/// depends on the recognizer:
///
/// - **`pattern`**: `None` → load the shared singleton with every
///   built-in pattern enabled. Pattern is always present in the
///   engine; the field is only here to let operators tune it (or
///   disable it via `enabled = false`).
/// - **`llm` / `ner` / `vlm`**: `None` → recognizer is not loaded,
///   no slot in the engine. These are network-bound and opt-in.
///
/// [`DetectionEngine`]: super::DetectionEngine
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct DetectionConfig {
    /// `[detection.pattern]` — pattern recognizer tuning. Omit the
    /// section to use the shared singleton with every built-in
    /// pattern enabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<PatternDetection>,
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
