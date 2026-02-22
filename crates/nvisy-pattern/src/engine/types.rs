//! Public types produced and consumed by the pattern engine.

use nvisy_core::data::{EntityCategory, EntityKind};

use crate::patterns::ContextRule;

/// How the match was produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DetectionSource {
    /// Matched by a compiled regular expression.
    Regex,
    /// Matched by Aho-Corasick dictionary lookup.
    Dictionary,
    /// Injected by the deny list (known sensitive value).
    DenyList,
}

/// A deny-list entry: a known sensitive value that must always be detected.
#[derive(Debug, Clone)]
pub struct DenyEntry {
    /// Entity category for the injected match.
    pub category: EntityCategory,
    /// Entity kind for the injected match.
    pub entity_kind: EntityKind,
}

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
    pub(crate) values: std::collections::HashSet<String>,
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
    pub fn contains(&self, value: &str) -> bool {
        self.values.contains(value)
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether the list is empty.
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

/// Exact-match deny list for forcing detection of known sensitive values.
///
/// If a deny-list value is found in the scanned text but was not already
/// matched by any regex or dictionary pattern, it is injected as a synthetic
/// [`PatternMatch`] with confidence `1.0` and source
/// [`DetectionSource::DenyList`].
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
    pub(crate) entries: std::collections::HashMap<String, DenyEntry>,
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
    pub fn contains(&self, value: &str) -> bool {
        self.entries.contains_key(value)
    }

    /// Look up the entry for a value.
    pub fn get(&self, value: &str) -> Option<&DenyEntry> {
        self.entries.get(value)
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the list is empty.
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

/// A single match produced by [`PatternEngine::scan_text`](super::PatternEngine::scan_text).
#[derive(Debug, Clone)]
pub struct PatternMatch {
    /// Name of the pattern that produced this match.
    pub pattern_name: String,
    /// Entity category of the match.
    pub category: EntityCategory,
    /// Entity kind of the match.
    pub entity_kind: EntityKind,
    /// Matched text.
    pub value: String,
    /// Byte offset of the match start in the input text.
    pub start: usize,
    /// Byte offset of the match end in the input text.
    pub end: usize,
    /// Confidence score assigned by the pattern definition.
    pub confidence: f64,
    /// Whether the match came from a regex or a dictionary.
    pub source: DetectionSource,
    /// Optional context rule for span-level co-occurrence scoring.
    pub context: Option<ContextRule>,
}

/// Errors that can occur while building a [`PatternEngine`](super::PatternEngine).
#[derive(Debug, thiserror::Error)]
pub enum PatternEngineError {
    /// A regex pattern string failed to compile.
    #[error("failed to compile regex for pattern '{name}': {source}")]
    RegexCompile {
        name: String,
        source: regex::Error,
    },
    /// A pattern references a dictionary that does not exist.
    #[error("pattern '{name}' references unknown dictionary '{dictionary}'")]
    UnknownDictionary {
        name: String,
        dictionary: String,
    },
    /// Failed to build an Aho-Corasick automaton.
    #[error("failed to build Aho-Corasick automaton for dictionary '{name}': {source}")]
    AhoCorasickBuild {
        name: String,
        source: aho_corasick::BuildError,
    },
    /// Failed to build the RegexSet pre-filter.
    #[error("failed to build RegexSet pre-filter: {0}")]
    RegexSetBuild(regex::Error),
}
