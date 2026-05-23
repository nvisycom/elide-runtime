//! [`Tokenizer`] trait and built-in implementations.
//!
//! Tokenization runs synchronously — splitting a string into tokens
//! is pure CPU and fast — but it is **fallible**: HuggingFace-backed
//! tokenizers can fail to encode certain inputs, and the trait
//! surface acknowledges that with a `Result` return.
//!
//! Pure-Unicode implementations cannot fail; their `tokenize` always
//! returns `Ok`.

#[cfg(feature = "onnx")]
mod hugging_face;
mod unicode;

#[cfg(feature = "onnx")]
#[cfg_attr(docsrs, doc(cfg(feature = "onnx")))]
pub use self::hugging_face::HfTokenizer;
pub use self::unicode::UnicodeTokenizer;
use crate::error::Result;

/// A single token produced by a [`Tokenizer`].
///
/// Byte offsets index the original text passed to [`Tokenizer::tokenize`].
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

/// ISO 639-1 codes the `iso`-featured `stop-words` build recognises.
///
/// Used to gate calls to [`get`] so we never reach the
/// crate's panic path on unknown codes. Keep this in sync with the
/// `stop-words` crate's `LANGUAGE` enum — adding more languages here
/// requires enabling additional features on the workspace
/// `stop-words` dependency.
///
/// [`get`]: stop_words::get
pub(crate) const SUPPORTED_STOPWORD_LANGUAGES: &[&str] = &[
    "ar", "da", "nl", "en", "fi", "fr", "de", "el", "hu", "id", "it", "no", "pt", "ro", "ru", "sl",
    "es", "sv", "tr",
];

/// Whether `code` (BCP-47 primary subtag / ISO 639-1) has a stopword
/// list available without panicking the `stop-words` crate.
pub(crate) fn is_stopword_language_supported(code: &str) -> bool {
    SUPPORTED_STOPWORD_LANGUAGES.contains(&code)
}

/// Split text into tokens.
///
/// Returned offsets are byte positions in the original text. The
/// concatenation of all token texts does **not** need to reproduce
/// the original input — whitespace and punctuation handling vary
/// across implementations.
///
/// # Errors
///
/// Implementations return [`Error::Tokenizer`] when they cannot
/// produce a token stream. Pure-Rust implementations like
/// [`UnicodeTokenizer`] never error in practice.
///
/// [`Error::Tokenizer`]: crate::Error::Tokenizer
pub trait Tokenizer: Send + Sync {
    /// Tokenize `text`.
    fn tokenize(&self, text: &str) -> Result<Vec<Token>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_is_supported() {
        assert!(is_stopword_language_supported("en"));
    }

    #[test]
    fn nonsense_codes_are_not_supported() {
        assert!(!is_stopword_language_supported("xx"));
        assert!(!is_stopword_language_supported(""));
        assert!(!is_stopword_language_supported("EN"));
    }
}
