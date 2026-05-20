//! [`DetectionContext`] — per-call input to
//! [`DetectionEngine::detect`] and every [`Recognizer`].
//!
//! Bundles the same shape `nvisy_nlp::Context` carries, plus the
//! [`ScanContext`] needed by pattern-backed recognizers. Each
//! recognizer reads the subset it cares about:
//!
//! - [`NerRecognizer`] honors `text`, `language`,
//!   `candidate_languages`, `entities`, `score_threshold`.
//! - [`PatternRecognizer`] reads `text` and `scan_context`
//!   (allow/deny/hints).
//! - [`LlmRecognizer`] reads `text` and honors its own per-build
//!   configuration; per-call overrides land on the recognizer at
//!   construction.
//!
//! `correlation_id` flows through the tracing span and isn't read
//! by recognizers themselves.
//!
//! [`DetectionEngine::detect`]: super::DetectionEngine::detect
//! [`Recognizer`]: crate::Recognizer
//! [`NerRecognizer`]: crate::NerRecognizer
//! [`PatternRecognizer`]: crate::PatternRecognizer
//! [`LlmRecognizer`]: crate::LlmRecognizer

use derive_builder::Builder;
use nvisy_ontology::entity::EntityKind;
use nvisy_ontology::primitive::LanguageTag;
use nvisy_pattern::ScanContext;
use uuid::Uuid;

/// Per-call input to [`DetectionEngine::detect`].
///
/// Lifetimes are hybrid: `text` is borrowed because it can be large
/// and is call-scoped; the optional lists and the [`ScanContext`]
/// are owned so the context can be passed around without lifetime
/// annotations spreading through callers.
///
/// [`DetectionEngine::detect`]: super::DetectionEngine::detect
#[derive(Debug, Clone, Builder)]
#[builder(
    name = "DetectionContextBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(error = "DetectionContextBuilderError")
)]
pub struct DetectionContext<'a> {
    /// The text to analyze.
    pub text: &'a str,

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
    pub scan_context: ScanContext,

    /// Correlation UUID propagated through the tracing span for
    /// this detection call.
    #[builder(default)]
    pub correlation_id: Option<Uuid>,
}

impl<'a> DetectionContext<'a> {
    /// Construct a context with only `text` set.
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            language: None,
            candidate_languages: None,
            entities: None,
            score_threshold: None,
            scan_context: ScanContext::default(),
            correlation_id: None,
        }
    }

    /// Start a typed builder. Equivalent to
    /// `DetectionContextBuilder::default()` but more discoverable
    /// from the context type.
    pub fn builder() -> DetectionContextBuilder<'a> {
        DetectionContextBuilder::default()
    }
}

impl<'a> From<&'a str> for DetectionContext<'a> {
    fn from(text: &'a str) -> Self {
        Self::new(text)
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
