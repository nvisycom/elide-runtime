//! [`Artifacts`] — output of a single [`NlpEngine::analyze`] call,
//! plus the [`Token`] type produced by [`Tokenizer`] impls.
//!
//! Composed from the outputs of independently-configured backends.
//! Fields are [`Option`] when produced by an optional component
//! (tokenizer). [`entities`](Artifacts::entities) is always
//! populated; [`language`](Artifacts::language) reflects either
//! caller-asserted or detected language, and may be `None` only when
//! detection on short or ambiguous text was inconclusive.
//!
//! [`NlpEngine::analyze`]: crate::engine::NlpEngine::analyze
//! [`Tokenizer`]: crate::tokenizer::Tokenizer

mod token;

pub use self::token::Token;

use std::collections::HashSet;

use nvisy_ontology::entity::Entities;
use nvisy_ontology::primitive::LanguageTag;

/// Result of one [`NlpEngine::analyze`] call.
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
/// [`NlpEngine::analyze`]: crate::engine::NlpEngine::analyze
#[derive(Debug, Clone)]
pub struct Artifacts {
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

    /// Lowercase surface forms of non-stopword tokens.
    ///
    /// Derived from `tokens` when both tokens and a stopword set are
    /// available. Useful for context-keyword lookup downstream.
    pub keywords: Option<HashSet<String>>,
}
