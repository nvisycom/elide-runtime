//! [`DenyList`]: forced detection of known sensitive values.

use std::collections::HashMap;
use std::sync::OnceLock;

use nvisy_ontology::entity::{EntityCategory, EntityKind};
use serde::{Deserialize, Serialize};

use super::deny_scanner::DenyScanner;

/// A deny-list rule: a known sensitive value that must always be detected.
///
/// Matches inject as synthetic detections with confidence `1.0` and
/// `RecognitionMethod::Pattern { kind: DenyList, .. }`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DenyRule {
    /// Entity category for the injected match.
    pub category: EntityCategory,
    /// Entity kind for the injected match.
    pub entity_kind: EntityKind,
}

/// Exact-match deny list for forcing detection of known sensitive values.
///
/// If a deny-list value is found in the scanned text but was not already
/// matched by any regex or dictionary pattern, it is injected as a synthetic
/// raw match with confidence `1.0` and `pattern_name: None`.
///
/// The first scan after construction compiles the values into an
/// Aho-Corasick automaton; subsequent scans reuse it.
///
/// Serializes as just `entries`; the cached automaton is rebuilt
/// lazily on the first scan after a round-trip.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DenyList {
    entries: HashMap<String, DenyRule>,
    #[serde(skip)]
    scanner: OnceLock<Option<DenyScanner>>,
}

impl DenyList {
    /// Create an empty deny list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a rule into this list.
    pub fn insert(&mut self, value: impl Into<String>, rule: DenyRule) {
        self.entries.insert(value.into(), rule);
        // Mutating invalidates any previously compiled scanner.
        self.scanner = OnceLock::new();
    }

    /// Look up the rule for a value.
    #[must_use]
    pub fn get(&self, value: &str) -> Option<&DenyRule> {
        self.entries.get(value)
    }

    /// Whether the list contains the given value.
    #[must_use]
    pub fn contains(&self, value: &str) -> bool {
        self.entries.contains_key(value)
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

    /// Return the lazily-compiled scanner, or `None` if the list is empty.
    ///
    /// Used internally by the pattern engine to scan text for deny-list
    /// values in O(n + matches) time.
    pub(crate) fn scanner(&self) -> Option<&DenyScanner> {
        self.scanner
            .get_or_init(|| DenyScanner::build(&self.entries))
            .as_ref()
    }
}

impl Clone for DenyList {
    fn clone(&self) -> Self {
        Self {
            entries: self.entries.clone(),
            scanner: OnceLock::new(),
        }
    }
}

impl<S, R> FromIterator<(S, R)> for DenyList
where
    S: Into<String>,
    R: Into<DenyRule>,
{
    fn from_iter<I: IntoIterator<Item = (S, R)>>(iter: I) -> Self {
        Self {
            entries: iter
                .into_iter()
                .map(|(s, r)| (s.into(), r.into()))
                .collect(),
            scanner: OnceLock::new(),
        }
    }
}

impl<S, R> Extend<(S, R)> for DenyList
where
    S: Into<String>,
    R: Into<DenyRule>,
{
    fn extend<I: IntoIterator<Item = (S, R)>>(&mut self, iter: I) {
        self.entries
            .extend(iter.into_iter().map(|(s, r)| (s.into(), r.into())));
        self.scanner = OnceLock::new();
    }
}
