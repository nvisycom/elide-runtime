//! [`Artifacts`] — output of a single [`Recognizer::recognize`] call.
//!
//! Composed from the outputs of independently-configured backends.
//! [`entities`] is always populated; [`languages`] preserves every
//! detection the policy produced (one per region for mixed-language
//! input, single-element for monolingual or caller-asserted answers,
//! empty when detection was inconclusive).
//!
//! [`Recognizer::recognize`]: super::Recognizer::recognize
//! [`entities`]: Artifacts::entities
//! [`languages`]: Artifacts::languages

use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::Text;
use nvisy_ontology::primitive::{LanguageDetection, LanguageTag};

/// NER output of one [`Recognizer::recognize`] call.
///
/// Carries the recognized entities plus the languages the engine
/// resolved for the input — either detected by the configured
/// [`LanguagePolicy`] or asserted by the caller.
///
/// [`Recognizer::recognize`]: super::Recognizer::recognize
/// [`LanguagePolicy`]: crate::language::LanguagePolicy
#[derive(Debug, Clone)]
pub struct Artifacts {
    /// Entities detected by the configured [`Backend`].
    ///
    /// Always populated (may be empty).
    ///
    /// [`Backend`]: crate::core::Backend
    pub entities: Vec<Entity<Text>>,

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
    /// [`LanguageProvenance::Asserted`]: nvisy_ontology::primitive::LanguageProvenance::Asserted
    /// [`dominant_language`]: Self::dominant_language
    pub languages: Vec<LanguageDetection>,
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
    /// [`LanguageSpan`]: nvisy_ontology::primitive::LanguageSpan
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
