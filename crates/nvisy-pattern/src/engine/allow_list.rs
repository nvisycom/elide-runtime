//! [`AllowList`] — exact-match suppression of known false positives.

use std::collections::HashSet;

/// Exact-match allow list for suppressing known false positives.
///
/// Values that appear in the allow list are silently dropped from
/// [`PatternEngine::scan_text`](super::PatternEngine::scan_text) results.
///
/// # Examples
///
/// ```rust,ignore
/// let allow = AllowList::new()
///     .with("123-45-6789")
///     .with("000-00-0000");
/// ```
#[derive(Debug, Clone, Default)]
pub struct AllowList {
    pub(crate) values: HashSet<String>,
}

impl AllowList {
    /// Create an empty allow list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a single value.
    pub fn with(mut self, value: impl Into<String>) -> Self {
        self.values.insert(value.into());
        self
    }

    /// Add multiple values.
    pub fn with_many(mut self, values: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.values.extend(values.into_iter().map(Into::into));
        self
    }

    /// Insert a value into this list.
    pub fn insert(&mut self, value: impl Into<String>) {
        self.values.insert(value.into());
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
