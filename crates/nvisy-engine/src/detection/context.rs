//! [`DetectionContext`] — per-call input to
//! [`DetectionEngine::run`] and every [`Recognizer`].
//!
//! Bundles the same shape `nvisy_ner::Context` carries, plus the
//! [`PatternContext`] needed by pattern-backed recognizers. Each
//! recognizer reads the subset it cares about:
//!
//! - [`NerRecognizer`] honors `text`, `language`,
//!   `candidate_languages`, `entities`.
//! - [`PatternRecognizer`] reads `text` and `scan_context`
//!   (allow/deny/hints).
//! - [`LlmRecognizer`] reads `text` and honors its own per-build
//!   configuration; per-call overrides land on the recognizer at
//!   construction.
//!
//! `correlation_id` flows through the tracing span and isn't read
//! by recognizers themselves.
//!
//! [`DetectionEngine::run`]: super::DetectionEngine::run
//! [`Recognizer`]: crate::Recognizer
//! [`NerRecognizer`]: crate::NerRecognizer
//! [`PatternRecognizer`]: crate::PatternRecognizer
//! [`LlmRecognizer`]: crate::LlmRecognizer

use derive_builder::Builder;
use nvisy_codec::handler::TextData;
use nvisy_ontology::entity::EntityKind;
use nvisy_ontology::primitive::LanguageTag;
use nvisy_pattern::filter::PatternContext;
use uuid::Uuid;

/// Per-call input to [`DetectionEngine::run`].
///
/// Fully owned (no lifetime parameter) so the engine can share it
/// across recognizer tasks via [`Arc`] for parallel dispatch.
/// `text` is a [`TextData`] — internally a `HipStr` — so the
/// shared clone is an atomic increment, not a copy of the source
/// bytes.
///
/// [`DetectionEngine::run`]: super::DetectionEngine::run
/// [`Arc`]: std::sync::Arc
#[derive(Debug, Clone, Builder)]
#[builder(
    name = "DetectionContextBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(error = "DetectionContextBuilderError")
)]
pub struct DetectionContext {
    /// The text to analyze. Cheap to clone (atomic incr on the
    /// inner `HipStr`, inline for short text).
    #[builder(setter(into))]
    pub text: TextData,

    /// Caller-asserted language. When `Some`, NER recognizers skip
    /// per-call language detection.
    #[builder(default)]
    pub language: Option<LanguageTag>,

    /// Restrict language detection to this subset. Ignored when
    /// `language` is `Some`.
    #[builder(default)]
    pub candidate_languages: Option<Vec<LanguageTag>>,

    /// Entity-kind allowlist. Recognizers that support post-filter
    /// drop entities of any kind outside this set.
    #[builder(default)]
    pub entities: Option<Vec<EntityKind>>,

    /// Minimum confidence threshold in `[0.0, 1.0]`. Recognizers
    /// that support post-filter drop entities below this score.
    #[builder(default)]
    pub score_threshold: Option<f64>,

    /// Allow/deny/hints for pattern-backed recognizers.
    /// Non-pattern recognizers ignore this field.
    #[builder(default)]
    pub scan_context: PatternContext,

    /// Correlation UUID propagated through the tracing span for
    /// this detection call.
    #[builder(default)]
    pub correlation_id: Option<Uuid>,
}

impl DetectionContext {
    /// Construct a context with only `text` set.
    pub fn new(text: impl Into<TextData>) -> Self {
        Self {
            text: text.into(),
            language: None,
            candidate_languages: None,
            entities: None,
            score_threshold: None,
            scan_context: PatternContext::default(),
            correlation_id: None,
        }
    }

    /// Start a typed builder. Equivalent to
    /// `DetectionContextBuilder::default()` but more discoverable
    /// from the context type.
    pub fn builder() -> DetectionContextBuilder {
        DetectionContextBuilder::default()
    }
}

impl From<TextData> for DetectionContext {
    fn from(text: TextData) -> Self {
        Self::new(text)
    }
}

impl From<&str> for DetectionContext {
    fn from(text: &str) -> Self {
        Self::new(TextData::from(text))
    }
}

impl From<String> for DetectionContext {
    fn from(text: String) -> Self {
        Self::new(TextData::from(text))
    }
}

/// Error returned by [`DetectionContextBuilder::build`] when a
/// required field is missing.
#[derive(Debug, thiserror::Error)]
#[error("DetectionContext build failed: {0}")]
pub struct DetectionContextBuilderError(String);

impl From<derive_builder::UninitializedFieldError> for DetectionContextBuilderError {
    fn from(err: derive_builder::UninitializedFieldError) -> Self {
        Self(format!("missing required field `{}`", err.field_name()))
    }
}
