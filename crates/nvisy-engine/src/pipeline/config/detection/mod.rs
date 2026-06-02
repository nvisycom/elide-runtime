//! Detection config: deployment-time `[detection.*]` configuration
//! the engine builds a [`RecognizerRegistry`] from at startup, plus
//! the per-request [`Detection`] plan node.
//!
//! Pattern detection is always-on (default registry + custom extras
//! aren't yet plan-configurable). NER is opt-in via
//! `[detection.ner]`. LLM and VLM sections are not currently wired
//! — those modules are parked pending rework to implement
//! [`nvisy_core::EntityRecognizer<M>`] directly.
//!
//! [`RecognizerRegistry`]: crate::detection::RecognizerRegistry

mod ner;
mod pattern;
mod plan;

pub use self::ner::{NerBackend, NerDetection};
pub use self::pattern::PatternDetection;
pub use self::plan::Detection;

/// Configuration for the
/// [`RecognizerRegistry`].
///
/// Each field maps to a `[detection.*]` section in `Nvisy.toml`.
/// Every field is `Option<_>` so missing sections are valid —
/// `None` means the recognizer is not loaded (or uses its always-on
/// default for `pattern`).
///
/// [`RecognizerRegistry`]: crate::detection::RecognizerRegistry
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct DetectionConfig {
    /// `[detection.pattern]` — pattern recognizer config. `None`
    /// uses the shipped registry with default settings (the
    /// recognizer is always-on).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<PatternDetection>,
    /// `[detection.ner]` — NER recognizer config bundle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ner: Option<NerDetection>,
}
