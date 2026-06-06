//! [`RecognizerInput<M>`]: per-call input for an
//! [`EntityRecognizer<M>`].
//!
//! Flat per-call surface for recognizers: the modality payload plus
//! the per-call concerns recognizers actually use (language hints,
//! candidate-language whitelist, uploader-supplied [`Hint<M>`]
//! regions, document-level labels, correlation id, and the typed
//! [`Artifacts`] bundle for shared NLP enrichment).
//!
//! Location is intentionally absent — recognizers emit entities in
//! modality-local coordinates and don't read a per-call location.
//! Extractors use [`Span<M>`] instead, which carries the location.
//!
//! [`Artifacts`]: crate::extraction::Artifacts
//! [`EntityRecognizer<M>`]: super::EntityRecognizer
//! [`Span<M>`]: crate::extraction::Span

use uuid::Uuid;

use super::Hint;
use crate::extraction::Artifacts;
use crate::modality::Modality;
use crate::primitive::LanguageTag;

/// Per-call input for an [`EntityRecognizer<M>`].
///
/// Bundles the modality payload with the per-call concerns
/// recognizers actually use; location lives on the
/// extractor-side [`Span<M>`] instead since recognizers don't need
/// it.
///
/// [`EntityRecognizer<M>`]: super::EntityRecognizer
/// [`Span<M>`]: crate::extraction::Span
#[derive(Debug)]
pub struct RecognizerInput<M: Modality> {
    /// Modality-specific payload (text bytes, image bytes + dims,
    /// …).
    pub data: M::Data,
    /// Shared per-call NLP enrichment, keyed by Rust type.
    /// Recognizers that don't care leave it empty.
    pub artifacts: Artifacts,
    /// Caller-asserted language. When `Some`, recognizers that
    /// support per-call language hinting (typically NER / LLM
    /// backends) skip their own detection.
    pub language: Option<LanguageTag>,
    /// Restrict language auto-detection to this subset when
    /// [`language`] is `None`. Empty means "any".
    ///
    /// [`language`]: Self::language
    pub candidate_languages: Vec<LanguageTag>,
    /// Uploader-supplied hint regions in modality-native coordinates.
    /// Recognizers that support hint adjudication (LLM-based NER, VLM)
    /// read this; recognizers that don't (pattern, dictionary) ignore
    /// it.
    pub hints: Vec<Hint<M>>,
    /// Document-level classification labels (e.g. `"medical"`,
    /// `"gdpr-request"`). Recognizers may use these to bias their
    /// behavior for domain-specific terms; those that don't ignore the
    /// field.
    pub labels: Vec<String>,
    /// Correlation UUID propagated through the tracing span for this
    /// call. Recognizer bodies do not read this directly; it's set
    /// on the span by the caller.
    pub correlation_id: Option<Uuid>,
}

impl<M: Modality> RecognizerInput<M> {
    /// Construct an input with only the modality payload set;
    /// every other field defaults to empty.
    pub fn new(data: M::Data) -> Self {
        Self {
            data,
            artifacts: Artifacts::new(),
            language: None,
            candidate_languages: Vec::new(),
            hints: Vec::new(),
            labels: Vec::new(),
            correlation_id: None,
        }
    }

    /// Replace the artifacts bundle.
    #[must_use]
    pub fn with_artifacts(mut self, artifacts: Artifacts) -> Self {
        self.artifacts = artifacts;
        self
    }

    /// Set the asserted language.
    #[must_use]
    pub fn with_language(mut self, language: LanguageTag) -> Self {
        self.language = Some(language);
        self
    }

    /// Set the candidate languages for auto-detection.
    #[must_use]
    pub fn with_candidate_languages(mut self, languages: Vec<LanguageTag>) -> Self {
        self.candidate_languages = languages;
        self
    }

    /// Attach uploader-supplied hint regions.
    #[must_use]
    pub fn with_hints(mut self, hints: Vec<Hint<M>>) -> Self {
        self.hints = hints;
        self
    }

    /// Attach document-level classification labels.
    #[must_use]
    pub fn with_labels(mut self, labels: Vec<String>) -> Self {
        self.labels = labels;
        self
    }

    /// Set the correlation id propagated through the tracing span.
    #[must_use]
    pub fn with_correlation_id(mut self, id: Uuid) -> Self {
        self.correlation_id = Some(id);
        self
    }

    /// Whether a recognizer rule scoped to `allowed` languages
    /// should run for this call.
    ///
    /// - An empty `allowed` list means the rule is language-agnostic
    ///   and always runs.
    /// - When `allowed` is non-empty and [`language`] is `Some(_)`,
    ///   the rule runs only if the hint is in the list.
    /// - When [`language`] is `None`, the rule still runs — we can't
    ///   disprove applicability without a hint.
    ///
    /// [`language`]: Self::language
    #[must_use]
    pub fn applies_to_language(&self, allowed: &[LanguageTag]) -> bool {
        if allowed.is_empty() {
            return true;
        }
        match self.language.as_ref() {
            Some(l) => allowed.iter().any(|a| a == l),
            None => true,
        }
    }
}
