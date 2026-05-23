//! [`Context`] — per-call input to [`NlpEngine::analyze`].
//!
//! Packages the text to analyze with optional knobs that Presidio's
//! `AnalyzerEngine.analyze()` accepts per call: an asserted language
//! (skips detection), a candidate-language set (restricts detection),
//! an entity-kind allowlist (post-filters the NER output), a
//! confidence floor (drops low-score entities), and a correlation
//! UUID for tracing.
//!
//! Lifetimes are hybrid: `text` is borrowed because it can be large
//! and is call-scoped; the small option lists are owned so the
//! context can be passed around without lifetime annotations on every
//! caller.
//!
//! Construct via:
//!
//! - `Context::from(text)` / `engine.analyze("text")` for the
//!   default "just analyze this text" case.
//! - [`ContextBuilder`] (via [`Context::builder`]) when one or more
//!   knobs need to be set. The builder uses `with_*` setters to
//!   match other builders in this crate.
//!
//! [`NlpEngine::analyze`]: super::NlpEngine::analyze

use derive_builder::Builder;
use nvisy_ontology::entity::EntityKind;
use nvisy_ontology::primitive::LanguageTag;
use uuid::Uuid;

/// Per-call input to [`NlpEngine::analyze`].
///
/// Construct via `Context::from(text)` for the bare "analyze this
/// text" case, or via [`Context::builder`] when several fields need
/// to be set explicitly.
///
/// [`NlpEngine::analyze`]: super::NlpEngine::analyze
#[derive(Debug, Clone, Builder)]
#[builder(
    name = "ContextBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(error = "ContextBuilderError")
)]
pub struct Context<'a> {
    /// The text to analyze.
    pub text: &'a str,

    /// Caller-asserted language. When `Some`, detection is skipped
    /// and the asserted value becomes the sole entry in
    /// [`Artifacts::languages`] with provenance
    /// [`LanguageProvenance::Asserted`].
    ///
    /// [`Artifacts::languages`]: crate::Artifacts::languages
    /// [`LanguageProvenance::Asserted`]: crate::language::LanguageProvenance::Asserted
    #[builder(default)]
    pub language: Option<LanguageTag>,

    /// Restrict language detection to this subset. Ignored when
    /// the `language` field is `Some`. The engine forwards this
    /// to [`LanguagePolicy::detector_for`] each call, so the policy
    /// decides what "restricted to this set" means concretely.
    ///
    /// [`LanguagePolicy::detector_for`]: crate::language::LanguagePolicy::detector_for
    #[builder(default)]
    pub candidate_languages: Option<Vec<LanguageTag>>,

    /// Entity-kind allowlist. When `Some`, entities of any kind
    /// outside this set are dropped after the NER backend runs.
    /// `None` keeps every entity the backend produced.
    #[builder(default)]
    pub entities: Option<Vec<EntityKind>>,

    /// Minimum confidence threshold in `[0.0, 1.0]`. Entities below
    /// this score are dropped. `None` keeps everything.
    #[builder(default)]
    pub score_threshold: Option<f64>,

    /// Correlation UUID propagated through the tracing span for this
    /// call. Not used for detection.
    #[builder(default)]
    pub correlation_id: Option<Uuid>,
}

impl<'a> Context<'a> {
    /// Construct a context with only `text` set. Equivalent to
    /// `Context::from(text)`; kept as a named constructor for
    /// callers that prefer it.
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            language: None,
            candidate_languages: None,
            entities: None,
            score_threshold: None,
            correlation_id: None,
        }
    }

    /// Start a typed builder.
    ///
    /// Equivalent to `ContextBuilder::default()` but more
    /// discoverable from the context type.
    pub fn builder() -> ContextBuilder<'a> {
        ContextBuilder::default()
    }
}

impl<'a> From<&'a str> for Context<'a> {
    fn from(text: &'a str) -> Self {
        Self::new(text)
    }
}

impl<'a> From<&'a String> for Context<'a> {
    fn from(text: &'a String) -> Self {
        Self::new(text.as_str())
    }
}

/// Error returned by [`ContextBuilder::build`] when a required
/// field is missing.
#[derive(Debug, thiserror::Error)]
#[error("Context build failed: {0}")]
pub struct ContextBuilderError(String);

impl From<derive_builder::UninitializedFieldError> for ContextBuilderError {
    fn from(err: derive_builder::UninitializedFieldError) -> Self {
        Self(format!("missing required field `{}`", err.field_name()))
    }
}
