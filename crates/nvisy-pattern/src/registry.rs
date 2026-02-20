//! Generic named-value registry backed by a `BTreeMap`.
//!
//! [`Registry<V>`] is the shared storage layer used by both
//! [`PatternRegistry`] and [`DictionaryRegistry`].  The `BTreeMap`
//! backend guarantees deterministic (alphabetical) iteration order.
//!
//! [`Registry<V>`]: crate::registry::Registry
//! [`PatternRegistry`]: crate::patterns::PatternRegistry
//! [`DictionaryRegistry`]: crate::dictionaries::DictionaryRegistry

use std::collections::BTreeMap;

/// A sorted map of named values with O(log n) lookup.
#[derive(Debug)]
pub struct Registry<V> {
    inner: BTreeMap<String, V>,
}

impl<V> Registry<V> {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    /// Insert a value under the given name, replacing any previous entry.
    pub fn insert(&mut self, name: String, value: V) {
        self.inner.insert(name, value);
    }

    /// Look up a value by name.
    pub fn get(&self, name: &str) -> Option<&V> {
        self.inner.get(name)
    }

    /// All values in alphabetical order by name.
    pub fn values(&self) -> Vec<&V> {
        self.inner.values().collect()
    }

    /// All names in alphabetical order.
    pub fn names(&self) -> Vec<&str> {
        self.inner.keys().map(|s| s.as_str()).collect()
    }

    /// Total number of entries.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl<V> Default for Registry<V> {
    fn default() -> Self {
        Self::new()
    }
}
