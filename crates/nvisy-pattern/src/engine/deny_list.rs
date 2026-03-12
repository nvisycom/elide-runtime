//! [`DenyList`] — forced detection of known sensitive values.

use std::collections::BTreeMap;

use nvisy_ontology::entity::{EntityCategory, EntityKind, RecognitionMethod};

/// A deny-list rule: a known sensitive value that must always be detected.
#[derive(Debug, Clone)]
pub struct DenyRule {
    /// Entity category for the injected match.
    pub category: EntityCategory,
    /// Entity kind for the injected match.
    pub entity_kind: EntityKind,
    /// Recognition method to assign to injected matches.
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
/// use nvisy_ontology::entity::{EntityCategory, EntityKind};
///
/// let deny = DenyList::new()
///     .with("John Doe", EntityCategory::PersonalIdentity, EntityKind::PersonName)
///     .with("ACME Corp", EntityCategory::PersonalIdentity, EntityKind::OrganizationName);
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

    /// Add a single rule with `RecognitionMethod::Dictionary` as the default method.
    pub fn with(
        mut self,
        value: impl Into<String>,
        category: EntityCategory,
        entity_kind: EntityKind,
    ) -> Self {
        self.entries.insert(
            value.into(),
            DenyRule {
                category,
                entity_kind,
                method: RecognitionMethod::Dictionary,
            },
        );
        self
    }

    /// Add a single rule with an explicit recognition method.
    pub fn with_method(
        mut self,
        value: impl Into<String>,
        category: EntityCategory,
        entity_kind: EntityKind,
        method: RecognitionMethod,
    ) -> Self {
        self.entries.insert(
            value.into(),
            DenyRule {
                category,
                entity_kind,
                method,
            },
        );
        self
    }

    /// Insert a rule into this list with `RecognitionMethod::Dictionary` as the default method.
    pub fn insert(
        &mut self,
        value: impl Into<String>,
        category: EntityCategory,
        entity_kind: EntityKind,
    ) {
        self.entries.insert(
            value.into(),
            DenyRule {
                category,
                entity_kind,
                method: RecognitionMethod::Dictionary,
            },
        );
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

impl<S: Into<String>> FromIterator<(S, EntityCategory, EntityKind)> for DenyList {
    fn from_iter<I: IntoIterator<Item = (S, EntityCategory, EntityKind)>>(iter: I) -> Self {
        let mut list = Self::new();
        for (value, category, entity_kind) in iter {
            list.insert(value, category, entity_kind);
        }
        list
    }
}
