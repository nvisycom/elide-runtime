//! [`NlpArtifacts`] — output of a single [`NlpEngine::analyze`] call.
//!
//! Composed from the outputs of independently-configured backends.
//! Fields are [`Option`] when produced by an optional component
//! (tokenizer). [`entities`](NlpArtifacts::entities) is always
//! populated; [`language`](NlpArtifacts::language) reflects either
//! caller-asserted or detected language, and may be `None` only when
//! detection on short or ambiguous text was inconclusive.
//!
//! [`NlpEngine::analyze`]: crate::engine::NlpEngine::analyze

use std::collections::HashSet;

use nvisy_ontology::entity::Entities;
use nvisy_ontology::primitive::LanguageTag;

use super::Token;

/// Result of one [`NlpEngine::analyze`] call.
///
/// Mirrors the field set Presidio's `NlpArtifacts` actually exposes to
/// downstream recognizers — entities + tokens + keywords + language —
/// stripped to what is consumed in practice and reshaped for typed
/// access.
///
/// Lemmas are intentionally absent in v1; see the design doc and
/// <https://github.com/nvisycom/runtime/issues/154> for the rationale
/// and the trigger conditions for revisiting.
///
/// [`NlpEngine::analyze`]: crate::engine::NlpEngine::analyze
#[derive(Debug, Clone)]
pub struct NlpArtifacts {
    /// Entities detected by the configured [`NerBackend`].
    ///
    /// Always populated (may be empty).
    ///
    /// [`NerBackend`]: crate::ner::NerBackend
    pub entities: Entities,

    /// Language asserted by the caller or detected by the configured
    /// [`LanguageDetector`].
    ///
    /// `None` when detection on short text was inconclusive *and* the
    /// caller did not supply an asserted language.
    ///
    /// [`LanguageDetector`]: crate::language::LanguageDetector
    pub language: Option<LanguageTag>,

    /// Token stream from the configured [`Tokenizer`], if any.
    ///
    /// [`Tokenizer`]: crate::tokenizer::Tokenizer
    pub tokens: Option<Vec<Token>>,

    /// Lowercase surface forms of non-stopword non-punctuation tokens.
    ///
    /// Derived from `tokens` when both tokens and a stopword set are
    /// available. Useful for context-keyword lookup downstream.
    pub keywords: Option<HashSet<String>>,
}
