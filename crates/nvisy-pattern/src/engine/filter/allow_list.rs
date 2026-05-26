//! [`AllowList`]: exact-match suppression of known false positives.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// Exact-match allow list for suppressing known false positives.
///
/// Values that appear in the allow list are silently dropped from
/// [`PatternEngine::scan`] results.
///
/// Populate via [`FromIterator`] or [`Extend`]:
///
/// ```rust,ignore
/// let allow: AllowList = ["123-45-6789", "000-00-0000"].into_iter().collect();
/// ```
///
/// Serializes transparently as a JSON array of strings.
///
/// [`PatternEngine::scan`]: crate::PatternEngine::scan
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AllowList {
    values: HashSet<String>,
}

impl AllowList {
    /// Create an empty allow list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the list contains the given value.
    #[must_use]
    pub fn contains(&self, value: &str) -> bool {
        self.values.contains(value)
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether the list is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

impl<S: Into<String>> FromIterator<S> for AllowList {
    fn from_iter<I: IntoIterator<Item = S>>(iter: I) -> Self {
        Self {
            values: iter.into_iter().map(Into::into).collect(),
        }
    }
}

impl<S: Into<String>> Extend<S> for AllowList {
    fn extend<I: IntoIterator<Item = S>>(&mut self, iter: I) {
        self.values.extend(iter.into_iter().map(Into::into));
    }
}
