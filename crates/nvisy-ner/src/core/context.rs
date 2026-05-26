//! [`NerContext`] — per-call configuration for [`NerEngine::analyze`].
//!
//! Packages the per-call knobs that `analyze()` accepts: an asserted
//! language (skips detection), a candidate-language set (restricts
//! detection), an entity-kind allowlist (post-filters the NER
//! output), and a correlation UUID for tracing.
//!
//! `text` is **not** a field — it's a separate argument to
//! [`NerEngine::analyze`]. Keeps the context cheap to clone and
//! reusable across multiple `analyze` calls.
//!
//! Construct via [`NerContext::default`] for the bare case, or via
//! [`NerContextBuilder`] when several fields need to be set.
//!
//! [`NerEngine`]: crate::NerEngine
//! [`NerEngine::analyze`]: crate::NerEngine::analyze

use derive_builder::Builder;
use nvisy_ontology::entity::EntityKind;
use nvisy_ontology::primitive::LanguageTag;
use uuid::Uuid;

/// Per-call configuration for [`NerEngine::analyze`].
///
/// [`NerEngine::analyze`]: crate::NerEngine::analyze
#[derive(Debug, Default, Clone, Builder)]
#[builder(
    name = "NerContextBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    default,
    build_fn(error = "NerContextBuilderError")
)]
pub struct NerContext {
    /// Caller-asserted language. When `Some`, detection is skipped
    /// and the asserted value becomes the sole entry in
    /// [`Artifacts::languages`] with provenance
    /// [`LanguageProvenance::Asserted`].
    ///
    /// [`Artifacts::languages`]: crate::Artifacts::languages
    /// [`LanguageProvenance::Asserted`]: nvisy_ontology::primitive::LanguageProvenance::Asserted
    pub language: Option<LanguageTag>,

    /// Restrict language detection to this subset. Ignored when
    /// the `language` field is `Some`. The engine forwards this
    /// to [`LanguagePolicy::detector_for`] each call, so the policy
    /// decides what "restricted to this set" means concretely.
    ///
    /// [`LanguagePolicy::detector_for`]: crate::language::LanguagePolicy::detector_for
    pub candidate_languages: Option<Vec<LanguageTag>>,

    /// Entity-kind allowlist. Threaded into zero-shot backends as
    /// the `requested_kinds` hint so the backend materialises
    /// labels for the asked-about kinds.
    ///
    /// Post-filtering on this list is the caller's responsibility —
    /// `analyze` returns whatever the backend produced for the
    /// requested kinds.
    pub entities: Option<Vec<EntityKind>>,

    /// Correlation UUID propagated through the tracing span for this
    /// call and (when set) into the backend via
    /// [`NerParams::correlation_id`].
    ///
    /// [`NerParams::correlation_id`]: crate::NerParams::correlation_id
    pub correlation_id: Option<Uuid>,
}

impl NerContext {
    /// Start a typed builder.
    ///
    /// Equivalent to `NerContextBuilder::default()` but more
    /// discoverable from the context type.
    pub fn builder() -> NerContextBuilder {
        NerContextBuilder::default()
    }
}

/// Error returned by [`NerContextBuilder::build`] when a required
/// field is missing.
#[derive(Debug, thiserror::Error)]
#[error("NerContext build failed: {0}")]
pub struct NerContextBuilderError(String);

impl From<derive_builder::UninitializedFieldError> for NerContextBuilderError {
    fn from(err: derive_builder::UninitializedFieldError) -> Self {
        Self(format!("missing required field `{}`", err.field_name()))
    }
}
