//! [`Artifacts`] — output of a single [`Engine::analyze`] call.
//!
//! Composed from the outputs of independently-configured backends.
//! Fields are [`Option`] when produced by an optional component
//! (tokenizer). [`entities`] is always populated; [`language`] reflects
//! either caller-asserted or detected language, and may be `None` only
//! when detection on short or ambiguous text was inconclusive.
//!
//! [`Engine::analyze`]: super::Engine::analyze
//! [`entities`]: Artifacts::entities
//! [`language`]: Artifacts::language

use std::collections::HashSet;

use nvisy_ontology::entity::Entities;
use nvisy_ontology::primitive::LanguageTag;

/// NLP output of one [`Engine::analyze`] call.
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
/// [`Engine::analyze`]: super::Engine::analyze
#[derive(Debug, Clone)]
pub struct Artifacts {
    /// Entities detected by the configured [`NerBackend`].
    ///
    /// Always populated (may be empty).
    ///
    /// [`NerBackend`]: crate::ner::NerBackend
    pub entities: Entities,

    /// Language asserted by the caller or detected by the engine's
    /// [`LanguagePolicy`].
    ///
    /// `None` when detection on short text was inconclusive *and*
    /// the caller did not supply an asserted language.
    ///
    /// [`LanguagePolicy`]: crate::language::LanguagePolicy
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

/// A single token produced by a [`Tokenizer`].
///
/// Byte offsets index the original text passed to
/// [`Engine::analyze`].
///
/// [`Tokenizer`]: crate::tokenizer::Tokenizer
/// [`Engine::analyze`]: super::Engine::analyze
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// Byte offset of the token start in the original text.
    pub start: usize,
    /// Byte offset of the token end in the original text.
    pub end: usize,
    /// Surface form of the token.
    pub text: String,
    /// Whether the token is a stopword per the tokenizer's configured
    /// stopword set. Always `false` when no stopword set is configured.
    pub is_stop: bool,
}
