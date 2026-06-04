//! [`StopwordSet`]: the resolved stopword list for one
//! [`RecognizerInput::artifacts`].
//!
//! Engines that have a stopword list for the artifact's dominant
//! language pre-resolve it once at `process_text` time and store it
//! on the artifact, so downstream consumers (the enhancer) don't
//! need to call back into the engine to ask `is_stopword(word, lang)`
//! per match. Engines without one set the field to
//! [`StopwordSet::empty`].
//!
//! The set is case-insensitive on ASCII (the lookup lowercases its
//! argument before checking); engines normalizing to a non-ASCII
//! language can override by populating with already-lowercased
//! tokens and querying through [`StopwordSet::contains_exact`].
//!
//! [`RecognizerInput::artifacts`]: crate::RecognizerInput::artifacts

use std::collections::HashSet;

/// Resolved stopword set carried on a
/// [`RecognizerInput::artifacts`] bundle.
///
/// [`RecognizerInput::artifacts`]: crate::RecognizerInput::artifacts
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StopwordSet {
    inner: HashSet<String>,
}

impl StopwordSet {
    /// Empty set — for engines without a stopword list, or for
    /// languages with no resolved list.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Construct from an iterator of stopwords. Inputs are
    /// ASCII-lowercased on insert so [`contains`]
    /// matches case-insensitively.
    ///
    /// [`contains`]: Self::contains
    pub fn from_iter_lowered<I, S>(iter: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self {
            inner: iter
                .into_iter()
                .map(|s| s.as_ref().to_ascii_lowercase())
                .collect(),
        }
    }

    /// Number of stopwords.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Whether the set is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Case-insensitive (ASCII) membership test. The query is
    /// lowercased before the lookup.
    #[must_use]
    pub fn contains(&self, word: &str) -> bool {
        let lowered = word.to_ascii_lowercase();
        self.inner.contains(&lowered)
    }

    /// Exact-match membership test — no lowering. Use when the
    /// caller has already normalized.
    #[must_use]
    pub fn contains_exact(&self, word: &str) -> bool {
        self.inner.contains(word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_insensitive_lookup() {
        let set = StopwordSet::from_iter_lowered(["the", "A", "of"]);
        assert!(set.contains("The"));
        assert!(set.contains("a"));
        assert!(set.contains("OF"));
        assert!(!set.contains("dog"));
    }

    #[test]
    fn empty_set_contains_nothing() {
        let set = StopwordSet::empty();
        assert!(!set.contains("the"));
        assert_eq!(set.len(), 0);
    }
}
