//! [`Token`] — a single token produced by a [`Tokenizer`].
//!
//! [`Tokenizer`]: crate::tokenizer::Tokenizer

/// A single token produced by a [`Tokenizer`].
///
/// Byte offsets index the original text passed to
/// [`NlpEngine::analyze`].
///
/// [`Tokenizer`]: crate::tokenizer::Tokenizer
/// [`NlpEngine::analyze`]: crate::engine::NlpEngine::analyze
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
