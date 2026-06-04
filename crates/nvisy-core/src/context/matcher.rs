//! [`KeywordMatcher`] strategy + the two shipped implementations.
//!
//! - [`SubstringMatcher`] — ASCII case-insensitive substring search
//!   over the raw text window. The fallback when no [`Tokens`] are
//!   present in `RecognizerInput.artifacts`.
//! - [`LemmaMatcher`] — matches keywords against lemmatized tokens
//!   stamped on `RecognizerInput.artifacts` as a [`Tokens`] entry by an
//!   upstream NLP engine. Recognizes morphological variants
//!   ("running" → "run", "SSNs" → "ssn") that substring matching
//!   misses, at the cost of needing a producer engine with
//!   lemmatization.
//!
//! Both implementations are stateless; the
//! [`ContextEnhancer`] owns one as a
//! configured strategy.
//!
//! [`Tokens`]: super::Tokens
//! [`ContextEnhancer`]: super::ContextEnhancer

use super::Tokens;

/// Decide whether any keyword from `keywords` fires within `window`.
///
/// The trait is the strategy slot that lets the enhancer swap raw
/// substring matching for lemma-aware matching (or a third-party
/// fuzzy/word-boundary implementation) without changing its core
/// pipeline.
///
/// Implementations receive both a raw `window` slice of the source
/// text (for substring strategies) and an optional `tokens` view
/// (for token/lemma strategies). Either or both may be ignored.
pub trait KeywordMatcher: Send + Sync {
    /// `true` if at least one keyword from `keywords` appears in
    /// the input. `window` is the raw text slice surrounding the
    /// entity match; `tokens` is the subset of [`Tokens`] covering
    /// that same range when an upstream NLP engine produced one,
    /// `None` otherwise.
    ///
    /// [`Tokens`]: super::Tokens
    fn any_match(&self, window: &str, tokens: Option<&Tokens>, keywords: &[String]) -> bool;
}

/// ASCII case-insensitive substring matcher. The default — used
/// whenever no [`Tokens`] were stamped on `RecognizerInput.artifacts`, or
/// whenever the caller explicitly picks raw matching.
///
/// Fast, allocation-light, permissive: the keyword `"email"` fires
/// inside `"MyEmailAddress"`. Ignores the `tokens` argument.
///
/// [`Tokens`]: super::Tokens
#[derive(Debug, Clone, Copy, Default)]
pub struct SubstringMatcher;

impl KeywordMatcher for SubstringMatcher {
    fn any_match(&self, window: &str, _tokens: Option<&Tokens>, keywords: &[String]) -> bool {
        let lowered = window.to_ascii_lowercase();
        keywords
            .iter()
            .any(|kw| lowered.contains(&kw.to_ascii_lowercase()))
    }
}

/// Lemma-aware matcher. Compares each lemma in `tokens` against the
/// keyword list with ASCII case-insensitive equality.
///
/// Falls back to [`SubstringMatcher`] semantics when `tokens` is
/// `None` (no shared NLP artifact was produced) so the enhancer
/// can be wired uniformly regardless of whether a given scan had
/// artifacts.
///
/// Recognizes morphological variants the substring matcher cannot:
/// `"running" → "run"`, `"dogs" → "dog"`, `"SSNs" → "ssn"`. Cost
/// is one lowercase per keyword + one lowercase per lemma per
/// match attempt.
#[derive(Debug, Clone, Copy, Default)]
pub struct LemmaMatcher;

impl KeywordMatcher for LemmaMatcher {
    fn any_match(&self, window: &str, tokens: Option<&Tokens>, keywords: &[String]) -> bool {
        let Some(tokens) = tokens else {
            return SubstringMatcher.any_match(window, None, keywords);
        };
        let lowered_keywords: Vec<String> =
            keywords.iter().map(|k| k.to_ascii_lowercase()).collect();
        tokens.iter().any(|tok| {
            let lemma = tok.lemma.to_ascii_lowercase();
            lowered_keywords.iter().any(|kw| kw == &lemma)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::Token;
    use super::*;

    #[test]
    fn substring_matches_case_insensitively() {
        let m = SubstringMatcher;
        assert!(m.any_match("Your SSN: 123", None, &["ssn".into()]));
        assert!(m.any_match(
            "the SOCIAL SECURITY number",
            None,
            &["social security".into()]
        ));
        assert!(!m.any_match("nothing here", None, &["ssn".into()]));
    }

    #[test]
    fn substring_is_permissive() {
        let m = SubstringMatcher;
        assert!(m.any_match("MyEmailAddress", None, &["email".into()]));
    }

    #[test]
    fn lemma_matches_morph_variants() {
        // tokens with lemmatization: "running" → "run", "dogs" → "dog"
        let tokens = Tokens::new(vec![
            Token::from_text("the", 0..3),
            Token::from_text("running", 4..11).with_lemma("run"),
            Token::from_text("dogs", 12..16).with_lemma("dog"),
        ]);
        let m = LemmaMatcher;
        assert!(m.any_match("", Some(&tokens), &["run".into()]));
        assert!(m.any_match("", Some(&tokens), &["dog".into()]));
        assert!(!m.any_match("", Some(&tokens), &["cat".into()]));
    }

    #[test]
    fn lemma_falls_back_to_substring_without_tokens() {
        let m = LemmaMatcher;
        // No artifacts → fall back to substring matching.
        assert!(m.any_match("Your SSN: 123", None, &["ssn".into()]));
    }
}
