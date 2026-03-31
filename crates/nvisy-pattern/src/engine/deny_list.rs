//! [`DenyList`]: forced detection of known sensitive values.

use std::collections::BTreeMap;

use nvisy_ontology::entity::{EntityCategory, EntityKind, RecognitionMethod};

/// A deny-list rule: a known sensitive value that must always be detected.
#[derive(Debug, Clone)]
pub struct DenyRule {
    /// Entity category for the injected match.
    pub category: EntityCategory,
    /// Entity kind for the injected match.
    pub entity_kind: EntityKind,
    /// Recognition method carried from the original detection source.
    pub method: RecognitionMethod,
}

/// Exact-match deny list for forcing detection of known sensitive values.
///
/// If a deny-list value is found in the scanned text but was not already
/// matched by any regex or dictionary pattern, it is injected as a synthetic
/// [`RawMatch`](super::RawMatch) with confidence `1.0` and
/// `pattern_name: None`.
///
/// # Examples
///
/// ```rust,ignore
/// use nvisy_ontology::entity::{EntityCategory, EntityKind, RecognitionMethod};
///
/// let deny = DenyList::new()
///     .with("John Doe", DenyRule {
///         category: EntityCategory::PersonalIdentity,
///         entity_kind: EntityKind::PersonName,
///         method: RecognitionMethod::ner_anonymous(),
///     });
/// ```
#[derive(Debug, Clone, Default)]
pub struct DenyList {
    pub(crate) entries: BTreeMap<String, DenyRule>,
}

impl DenyList {
    /// Create an empty deny list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a single rule (builder style).
    pub fn with(mut self, value: impl Into<String>, rule: DenyRule) -> Self {
        self.entries.insert(value.into(), rule);
        self
    }

    /// Insert a rule into this list.
    pub fn insert(&mut self, value: impl Into<String>, rule: DenyRule) {
        self.entries.insert(value.into(), rule);
    }

    /// Whether the list contains the given value.
    #[must_use]
    pub fn contains(&self, value: &str) -> bool {
        self.entries.contains_key(value)
    }

    /// Look up the rule for a value.
    #[must_use]
    pub fn get(&self, value: &str) -> Option<&DenyRule> {
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

    /// Iterate over (value, rule) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &DenyRule)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }
}
