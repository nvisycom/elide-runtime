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
mod stopword_lang;
mod unicode;

pub use self::hugging_face::HfTokenizer;
pub use self::unicode::UnicodeTokenizer;

pub(crate) use self::stopword_lang::is_supported;

use crate::artifacts::Token;
use crate::error::Result;

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
