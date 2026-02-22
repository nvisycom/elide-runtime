//! [`DenyList`] — forced detection of known sensitive values.

use std::collections::HashMap;

use nvisy_core::data::{EntityCategory, EntityKind};

/// A deny-list entry: a known sensitive value that must always be detected.
#[derive(Debug, Clone)]
pub struct DenyEntry {
    /// Entity category for the injected match.
    pub category: EntityCategory,
    /// Entity kind for the injected match.
    pub entity_kind: EntityKind,
}

/// Exact-match deny list for forcing detection of known sensitive values.
///
/// If a deny-list value is found in the scanned text but was not already
/// matched by any regex or dictionary pattern, it is injected as a synthetic
/// [`PatternMatch`](super::PatternMatch) with confidence `1.0` and source
/// [`DetectionSource::DenyList`](super::DetectionSource::DenyList).
///
/// # Examples
///
/// ```rust,ignore
/// use nvisy_core::data::{EntityCategory, EntityKind};
///
/// let deny = DenyList::new()
///     .with("John Doe", EntityCategory::Pii, EntityKind::PersonName)
///     .with("ACME Corp", EntityCategory::Pii, EntityKind::Organization);
/// ```
#[derive(Debug, Clone, Default)]
pub struct DenyList {
    pub(crate) entries: HashMap<String, DenyEntry>,
}

impl DenyList {
    /// Create an empty deny list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a single entry.
    pub fn with(
        mut self,
        value: impl Into<String>,
        category: EntityCategory,
        entity_kind: EntityKind,
    ) -> Self {
        self.entries
            .insert(value.into(), DenyEntry { category, entity_kind });
        self
    }

    /// Insert an entry into this list.
    pub fn insert(
        &mut self,
        value: impl Into<String>,
        category: EntityCategory,
        entity_kind: EntityKind,
    ) {
        self.entries
            .insert(value.into(), DenyEntry { category, entity_kind });
    }

    /// Whether the list contains the given value.
    #[must_use]
    pub fn contains(&self, value: &str) -> bool {
        self.entries.contains_key(value)
    }

    /// Look up the entry for a value.
    #[must_use]
    pub fn get(&self, value: &str) -> Option<&DenyEntry> {
        self.entries.get(value)
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the list is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over (value, entry) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &DenyEntry)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }
}

impl<S: Into<String>> FromIterator<(S, EntityCategory, EntityKind)> for DenyList {
    fn from_iter<I: IntoIterator<Item = (S, EntityCategory, EntityKind)>>(iter: I) -> Self {
        let mut list = Self::new();
        for (value, category, entity_kind) in iter {
            list.insert(value, category, entity_kind);
        }
        list
    }
}
