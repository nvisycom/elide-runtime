//! [`KeywordMatcher`]: strategy for deciding whether any of a
//! keyword list appears within a text window.
//!
//! Ships with one default impl, [`SubstringMatcher`], which performs
//! a case-insensitive ASCII substring search. Consumers wanting a
//! different policy (word-boundary, lemma-based, fuzzy, …) implement
//! [`KeywordMatcher`] themselves and pass the result to
//! [`ContextEnhancerBuilder::with_matcher`](super::ContextEnhancerBuilder::with_matcher).

/// Decide whether any keyword in `keywords` appears in `window`.
pub trait KeywordMatcher: Send + Sync {
    /// `true` if at least one keyword from `keywords` is present in
    /// `window`. `window` is the surrounding text slice; the
    /// implementation chooses how strictly to match (substring,
    /// word-boundary, lemma, …).
    fn any_match(&self, window: &str, keywords: &[String]) -> bool;
}

/// ASCII case-insensitive substring matcher — the default if no
/// explicit matcher is supplied via the
/// [`ContextEnhancerBuilder`](super::ContextEnhancerBuilder).
///
/// Fast, allocation-light (one `to_ascii_lowercase` per call), and
/// permissive: the keyword `"email"` matches inside
/// `"MyEmailAddress"`. Replace with a custom [`KeywordMatcher`]
/// when stricter semantics are needed.
#[derive(Debug, Clone, Copy, Default)]
pub struct SubstringMatcher;

impl KeywordMatcher for SubstringMatcher {
    fn any_match(&self, window: &str, keywords: &[String]) -> bool {
        let lowered = window.to_ascii_lowercase();
        keywords
            .iter()
            .any(|kw| lowered.contains(&kw.to_ascii_lowercase()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substring_matches_case_insensitively() {
        let m = SubstringMatcher;
        assert!(m.any_match("Your SSN: 123", &["ssn".into()]));
        assert!(m.any_match("the SOCIAL SECURITY number", &["social security".into()]));
        assert!(!m.any_match("nothing here", &["ssn".into()]));
    }

    #[test]
    fn substring_is_permissive() {
        let m = SubstringMatcher;
        // a substring match doesn't care about word boundaries — even
        // the keyword embedded in a larger word counts.
        assert!(m.any_match("MyEmailAddress", &["email".into()]));
    }
}
