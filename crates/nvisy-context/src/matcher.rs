//! [`KeywordMatcher`] strategy + the two shipped implementations.
//!
//! - [`SubstringMatcher`] — ASCII case-insensitive substring search
//!   over the raw text window. The fallback when no token artifact
//!   is present on `RecognizerInput.artifacts`.
//! - [`LemmaMatcher`] — matches keywords against lemmatized tokens
//!   the upstream NLP engine stamped on `RecognizerInput.artifacts`
//!   as a [`Tokens`] entry. Recognizes morphological variants
//!   ("running" → "run", "SSNs" → "ssn") substring matching misses.
//!
//! Both implementations are stateless; the [`Enhancer`] owns one
//! as a configured strategy.
//!
//! [`Tokens`]: super::Tokens
//! [`Enhancer`]: super::Enhancer

use hipstr::HipStr;

use super::Token;

/// Decide whether any keyword from `keywords` fires within the
/// candidate region around an entity match.
///
/// The strategy slot that lets the enhancer swap raw substring
/// matching for lemma-aware matching (or a third-party
/// fuzzy/word-boundary implementation) without changing its core
/// pipeline.
///
/// Implementations receive both a raw `window` slice of the source
/// text (for substring strategies) and the `tokens` covering that
/// same range (for token/lemma strategies). Either or both may be
/// ignored; `tokens` is empty when no NLP engine produced a token
/// artifact.
pub trait KeywordMatcher: Send + Sync {
    /// `true` if at least one keyword from `keywords` appears in
    /// the input.
    fn any_match(&self, window: &str, tokens: &[Token], keywords: &[HipStr<'static>]) -> bool;
}

/// ASCII case-insensitive substring matcher. The default —
/// runs whenever no token artifact was stamped on
/// `RecognizerInput.artifacts`, or whenever the caller explicitly
/// picks raw matching.
///
/// Fast, allocation-light, permissive: the keyword `"email"` fires
/// inside `"MyEmailAddress"`. Ignores the `tokens` argument.
#[derive(Debug, Clone, Copy, Default)]
pub struct SubstringMatcher;

impl KeywordMatcher for SubstringMatcher {
    fn any_match(&self, window: &str, _tokens: &[Token], keywords: &[HipStr<'static>]) -> bool {
        let lowered = window.to_ascii_lowercase();
        keywords
            .iter()
            .any(|kw| lowered.contains(kw.as_str().to_ascii_lowercase().as_str()))
    }
}

/// Lemma-aware matcher. Compares each lemma in `tokens` against
/// the keyword list with ASCII case-insensitive equality.
///
/// Falls back to [`SubstringMatcher`] semantics when `tokens` is
/// empty (no shared NLP artifact was produced) so the enhancer
/// runs uniformly regardless of whether the upstream pass emitted
/// tokens.
///
/// Recognizes morphological variants the substring matcher cannot:
/// `"running" → "run"`, `"dogs" → "dog"`, `"SSNs" → "ssn"`. Cost
/// is one lowercase per keyword + one lowercase per lemma per
/// match attempt.
#[derive(Debug, Clone, Copy, Default)]
pub struct LemmaMatcher;

impl KeywordMatcher for LemmaMatcher {
    fn any_match(&self, window: &str, tokens: &[Token], keywords: &[HipStr<'static>]) -> bool {
        if tokens.is_empty() {
            return SubstringMatcher.any_match(window, tokens, keywords);
        }
        let lowered_keywords: Vec<String> = keywords
            .iter()
            .map(|k| k.as_str().to_ascii_lowercase())
            .collect();
        tokens.iter().any(|tok| {
            let lemma = tok.lemma.as_str().to_ascii_lowercase();
            lowered_keywords.contains(&lemma)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kws(items: &[&'static str]) -> Vec<HipStr<'static>> {
        items.iter().copied().map(HipStr::from).collect()
    }

    #[test]
    fn substring_matches_case_insensitively() {
        let m = SubstringMatcher;
        assert!(m.any_match("Your SSN: 123", &[], &kws(&["ssn"])));
        assert!(m.any_match(
            "the SOCIAL SECURITY number",
            &[],
            &kws(&["social security"])
        ));
        assert!(!m.any_match("nothing here", &[], &kws(&["ssn"])));
    }

    #[test]
    fn substring_is_permissive() {
        let m = SubstringMatcher;
        assert!(m.any_match("MyEmailAddress", &[], &kws(&["email"])));
    }

    #[test]
    fn lemma_matches_morph_variants() {
        let tokens = vec![
            Token::from_text("the", 0..3),
            Token::from_text("running", 4..11).with_lemma("run"),
            Token::from_text("dogs", 12..16).with_lemma("dog"),
        ];
        let m = LemmaMatcher;
        assert!(m.any_match("", &tokens, &kws(&["run"])));
        assert!(m.any_match("", &tokens, &kws(&["dog"])));
        assert!(!m.any_match("", &tokens, &kws(&["cat"])));
    }

    #[test]
    fn lemma_falls_back_to_substring_without_tokens() {
        let m = LemmaMatcher;
        assert!(m.any_match("Your SSN: 123", &[], &kws(&["ssn"])));
    }
}
