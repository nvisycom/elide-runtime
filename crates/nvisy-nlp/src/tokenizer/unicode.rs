//! [`UnicodeTokenizer`] — model-free tokenizer over Unicode word
//! boundaries.
//!
//! Wraps [`unicode-segmentation`] to produce one [`Token`] per Unicode
//! word, with `is_stop` driven by an optional stopword set (loaded
//! via the `stop-words` crate when a language is configured).
//!
//! Use this when you want token offsets without depending on a model.
//!
//! [`unicode-segmentation`]: https://docs.rs/unicode-segmentation

use std::collections::HashSet;

use nvisy_ontology::primitive::LanguageTag;
use unicode_segmentation::UnicodeSegmentation;

use super::{Tokenizer, is_stopword_language_supported};
use crate::engine::Token;
use crate::error::Result;

/// Unicode-segmentation tokenizer.
///
/// Construct with [`new`] for a stopword-free tokenizer or
/// [`with_language`] to load the matching stopword list from
/// [`stop-words`].
///
/// [`new`]: Self::new
/// [`with_language`]: Self::with_language
/// [`stop-words`]: https://crates.io/crates/stop-words
#[derive(Debug, Clone, Default)]
pub struct UnicodeTokenizer {
    stopwords: Option<HashSet<String>>,
}

impl UnicodeTokenizer {
    /// Create a tokenizer with no stopword set (every token's
    /// `is_stop` will be `false`).
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a tokenizer that consults the [`stop-words`] list for
    /// the given language. The lookup is by BCP-47 primary subtag —
    /// `en-US` and `en` share the same English list.
    ///
    /// Returns `None` if the `stop-words` crate doesn't recognise the
    /// primary subtag. The recognised set depends on `stop-words`'s
    /// feature flags; with the workspace default (`iso`) the common
    /// European languages are available — see the crate's docs for
    /// the full list.
    ///
    /// [`stop-words`]: https://crates.io/crates/stop-words
    pub fn with_language(lang: &LanguageTag) -> Option<Self> {
        let primary = lang.primary_language();
        if !is_stopword_language_supported(primary) {
            return None;
        }
        let words = stop_words::get(primary);
        Some(Self {
            stopwords: Some(words.iter().map(|s| s.to_lowercase()).collect()),
        })
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

impl Tokenizer for UnicodeTokenizer {
    fn tokenize(&self, text: &str) -> Result<Vec<Token>> {
        let tokens = text
            .unicode_word_indices()
            .map(|(start, word)| {
                let lowered = word.to_lowercase();
                Token {
                    start,
                    end: start + word.len(),
                    text: word.to_owned(),
                    is_stop: self.is_stop(&lowered),
                }
            })
            .collect();
        Ok(tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_unicode_word_boundaries() {
        let tok = UnicodeTokenizer::new();
        let tokens = tok.tokenize("Hello, world!").unwrap();
        let texts: Vec<&str> = tokens.iter().map(|t| t.text.as_str()).collect();
        // unicode_word_indices skips standalone punctuation.
        assert_eq!(texts, vec!["Hello", "world"]);
    }

    #[test]
    fn offsets_are_byte_positions() {
        let tok = UnicodeTokenizer::new();
        let text = "abc def";
        let tokens = tok.tokenize(text).unwrap();
        assert_eq!(tokens.len(), 2);
        assert_eq!(&text[tokens[0].start..tokens[0].end], "abc");
        assert_eq!(&text[tokens[1].start..tokens[1].end], "def");
    }

    #[test]
    fn stopwords_flagged_when_loaded() {
        let lang: LanguageTag = "en".parse().unwrap();
        let tok = UnicodeTokenizer::with_language(&lang).expect("english stopwords");
        let tokens = tok.tokenize("the quick brown fox").unwrap();
        let stop: Vec<&str> = tokens
            .iter()
            .filter(|t| t.is_stop)
            .map(|t| t.text.as_str())
            .collect();
        assert!(stop.contains(&"the"), "expected 'the' to be flagged stop");
    }

    #[test]
    fn no_stopwords_without_language() {
        let tok = UnicodeTokenizer::new();
        let tokens = tok.tokenize("the quick brown fox").unwrap();
        assert!(tokens.iter().all(|t| !t.is_stop));
    }

    #[test]
    fn with_language_unknown_returns_none() {
        let lang: LanguageTag = "xx".parse().unwrap();
        assert!(UnicodeTokenizer::with_language(&lang).is_none());
    }
}
