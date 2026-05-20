//! [`HfTokenizer`] — wraps HuggingFace's [`tokenizers`] crate.
//!
//! Useful when downstream code needs token offsets aligned with a
//! specific model's tokenization (the canonical case being
//! [`OrtNerBackend`]'s alignment). For dependency-free tokenization
//! over Unicode word boundaries, use [`UnicodeTokenizer`] instead.
//!
//! [`OrtNerBackend`]: crate::ner::OrtNerBackend
//! [`UnicodeTokenizer`]: super::UnicodeTokenizer

use std::collections::HashSet;
use std::path::Path;

use nvisy_ontology::primitive::LanguageTag;
use tokenizers::Tokenizer as InnerTokenizer;

use super::{Tokenizer, is_stopword_language_supported};
use crate::artifacts::Token;
use crate::error::{Error, Result};

/// A [`Tokenizer`] backed by a HuggingFace `tokenizer.json` file.
///
/// Construction loads the tokenizer eagerly so subsequent
/// [`tokenize`] calls don't repeat work.
///
/// # Limitations on `is_stop`
///
/// Subword tokenizers (BPE, WordPiece, Unigram) emit fragments like
/// `"##ing"` or `"▁the"`, not whole words. The `is_stop` flag is
/// computed by lowercasing each emitted token and looking it up in
/// the configured stopword set — which only matches when the token
/// happens to equal a stopword surface form. For most subword models
/// `is_stop` will be `false` even on real stopwords.
///
/// If you need word-level stopword filtering, run [`UnicodeTokenizer`]
/// alongside or do the filtering post-hoc on the source text.
///
/// [`tokenize`]: Self::tokenize
/// [`UnicodeTokenizer`]: super::UnicodeTokenizer
pub struct HfTokenizer {
    inner: InnerTokenizer,
    stopwords: Option<HashSet<String>>,
}

impl HfTokenizer {
    /// Load a tokenizer from a `tokenizer.json` path, with no
    /// stopword set attached.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Tokenizer`] when the file can't be read or
    /// parsed.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let inner = InnerTokenizer::from_file(path)
            .map_err(|e| Error::Tokenizer(format!("{}: {e}", path.display())))?;
        Ok(Self {
            inner,
            stopwords: None,
        })
    }

    /// Load a tokenizer and attach the stopword set for `lang` in one
    /// call.
    ///
    /// Returns the loaded tokenizer with the stopword set when
    /// `lang`'s primary subtag is among those `stop-words` recognises;
    /// otherwise returns the tokenizer without a stopword set
    /// (mirroring [`UnicodeTokenizer::with_language`]'s semantics:
    /// unknown language ≠ failure to load).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Tokenizer`] only if the tokenizer file
    /// itself can't be loaded.
    ///
    /// [`UnicodeTokenizer::with_language`]: super::UnicodeTokenizer::with_language
    pub fn with_language(path: impl AsRef<Path>, lang: &LanguageTag) -> Result<Self> {
        let mut tok = Self::from_file(path)?;
        let primary = lang.primary_language();
        if is_stopword_language_supported(primary) {
            let words = stop_words::get(primary);
            tok.stopwords = Some(words.iter().map(|s| s.to_lowercase()).collect());
        }
        Ok(tok)
    }

    /// Attach a custom stopword set, replacing any existing one.
    pub fn with_stopwords(mut self, stopwords: HashSet<String>) -> Self {
        self.stopwords = Some(stopwords);
        self
    }

    fn is_stop(&self, lowered: &str) -> bool {
        self.stopwords
            .as_ref()
            .is_some_and(|set| set.contains(lowered))
    }
}

impl Tokenizer for HfTokenizer {
    fn tokenize(&self, text: &str) -> Result<Vec<Token>> {
        let encoding = self
            .inner
            .encode(text, false)
            .map_err(|e| Error::Tokenizer(e.to_string()))?;
        let offsets = encoding.get_offsets();
        let tokens = encoding
            .get_tokens()
            .iter()
            .zip(offsets.iter())
            .map(|(text, &(start, end))| {
                let lowered = text.to_lowercase();
                Token {
                    start,
                    end,
                    text: text.clone(),
                    is_stop: self.is_stop(&lowered),
                }
            })
            .collect();
        Ok(tokens)
    }
}

impl std::fmt::Debug for HfTokenizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HfTokenizer")
            .field("stopwords", &self.stopwords.as_ref().map(HashSet::len))
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `HfTokenizer::from_file` against a missing path should surface
    /// `Error::Tokenizer` (not panic).
    #[test]
    fn missing_file_returns_error() {
        let err = HfTokenizer::from_file("/nonexistent/tokenizer.json").expect_err("should error");
        assert!(matches!(err, Error::Tokenizer(_)));
    }

    #[test]
    fn with_language_missing_file_returns_error() {
        let lang: LanguageTag = "en".parse().unwrap();
        let err = HfTokenizer::with_language("/nonexistent/tokenizer.json", &lang)
            .expect_err("should error");
        assert!(matches!(err, Error::Tokenizer(_)));
    }
}
