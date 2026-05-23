//! [`Tokenizer`] trait and built-in implementations.
//!
//! Tokenization runs synchronously — splitting a string into tokens
//! is pure CPU and fast — but it is **fallible**: HuggingFace-backed
//! tokenizers can fail to encode certain inputs, and the trait
//! surface acknowledges that with a `Result` return.
//!
//! Pure-Unicode implementations cannot fail; their `tokenize` always
//! returns `Ok`.

mod hugging_face;
mod unicode;

pub use self::hugging_face::HfTokenizer;
pub use self::unicode::UnicodeTokenizer;
use crate::engine::Token;
use crate::error::Result;

/// ISO 639-1 codes the `iso`-featured `stop-words` build recognises.
///
/// Used to gate calls to [`stop_words::get`] so we never reach the
/// crate's panic path on unknown codes. Keep this in sync with the
/// `stop-words` crate's `LANGUAGE` enum — adding more languages here
/// requires enabling additional features on the workspace
/// `stop-words` dependency.
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
