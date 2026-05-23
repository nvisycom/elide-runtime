//! [`Artifacts`] — output of a single [`NlpEngine::analyze`] call.
//!
//! Composed from the outputs of independently-configured backends.
//! [`entities`] is always populated; [`languages`] preserves every
//! detection the policy produced (one per region for mixed-language
//! input, single-element for monolingual or caller-asserted answers,
//! empty when detection was inconclusive).
//!
//! [`NlpEngine::analyze`]: super::NlpEngine::analyze
//! [`entities`]: Artifacts::entities
//! [`languages`]: Artifacts::languages

use std::collections::HashSet;

use nvisy_ontology::entity::Entities;
use nvisy_ontology::primitive::LanguageTag;

use crate::language::LanguageDetection;
use crate::tokenizer::Token;

/// NLP output of one [`NlpEngine::analyze`] call.
///
/// Mirrors the field set Presidio's `NlpArtifacts` actually exposes to
/// downstream recognizers — entities + tokens + keywords + language —
/// stripped to what is consumed in practice and reshaped for typed
/// access.
///
/// Lemmas are intentionally absent in v1;
/// <https://github.com/nvisycom/runtime/issues/154> captures the
/// rationale and trigger conditions for revisiting.
///
/// [`NlpEngine::analyze`]: super::NlpEngine::analyze
#[derive(Debug, Clone)]
pub struct Artifacts {
    /// Entities detected by the configured [`NerBackend`].
    ///
    /// Always populated (may be empty).
    ///
    /// [`NerBackend`]: crate::ner::NerBackend
    pub entities: Entities,

    /// Languages asserted by the caller or detected by the engine's
    /// [`LanguagePolicy`].
    ///
    /// One entry per region for backends that segment mixed-language
    /// input (e.g. lingua's `detect_multiple_languages_of`); a single
    /// entry for monolingual answers and for caller-asserted languages
    /// (provenance [`LanguageProvenance::Asserted`]); empty when
    /// detection was inconclusive *and* the caller didn't assert.
    ///
    /// Callers that only care about the dominant language can use
    /// [`dominant_language`].
    ///
    /// [`LanguagePolicy`]: crate::language::LanguagePolicy
    /// [`LanguageProvenance::Asserted`]: crate::language::LanguageProvenance::Asserted
    /// [`dominant_language`]: Self::dominant_language
    pub languages: Vec<LanguageDetection>,

    /// Token stream from the configured [`Tokenizer`], if any.
    ///
    /// [`Tokenizer`]: crate::tokenizer::Tokenizer
    pub tokens: Option<Vec<Token>>,

    /// Lowercase surface forms of non-stopword tokens.
    ///
    /// Derived from `tokens` when both tokens and a stopword set are
    /// available. Useful for context-keyword lookup downstream.
    pub keywords: Option<HashSet<String>>,
}

impl Artifacts {
    /// The language covering the most bytes of the source text,
    /// breaking ties on detector confidence.
    ///
    /// Monolingual docs are trivial — the only detection wins.
    /// Mixed-language docs return the language whose [`LanguageSpan`]
    /// covers the most bytes; if two languages cover the same number
    /// of bytes, the one with the higher [`Confidence`] wins (entries
    /// without confidence sort below those with one).
    ///
    /// Detections without a `span` are treated as covering the whole
    /// document — a single caller-asserted entry therefore always
    /// wins. Returns `None` iff [`languages`] is empty.
    ///
    /// Clones the tag; the vec itself is untouched.
    ///
    /// [`languages`]: Self::languages
    /// [`LanguageSpan`]: crate::language::LanguageSpan
    /// [`Confidence`]: nvisy_ontology::primitive::Confidence
    pub fn dominant_language(&self) -> Option<LanguageTag> {
        self.languages
            .iter()
            .max_by(|a, b| {
                span_bytes(a)
                    .cmp(&span_bytes(b))
                    .then_with(|| confidence_key(a).total_cmp(&confidence_key(b)))
            })
            .map(|d| d.language.clone())
    }
}

/// Bytes a detection covers. `None`-span detections are treated as
/// covering the whole document (a sensible default for caller-
/// asserted and single-language detectors that don't track regions).
fn span_bytes(d: &LanguageDetection) -> usize {
    match d.span {
        Some(s) => s.end.saturating_sub(s.start),
        None => usize::MAX,
    }
}

/// Sortable confidence key. Missing confidence sorts below any real
/// value so a detection that *has* a score wins ties over one that
/// doesn't.
fn confidence_key(d: &LanguageDetection) -> f64 {
    d.confidence.map(|c| c.get()).unwrap_or(f64::NEG_INFINITY)
}
