//! [`DenyScanner`]: pre-compiled Aho-Corasick automaton over a
//! [`DenyList`]'s values, with a parallel index back to the matched
//! [`DenyRule`].
//!
//! Built lazily by [`DenyList::scanner`] on the first scan and
//! re-built when the list mutates. Crate-private: only the scan
//! phase consumes it.
//!
//! [`DenyList`]: super::DenyList
//! [`DenyList::scanner`]: super::DenyList::scanner
//! [`DenyRule`]: super::DenyRule

use std::collections::HashMap;
use std::fmt;

use aho_corasick::AhoCorasick;

use super::DenyRule;

/// Pre-compiled Aho-Corasick automaton over a deny list.
pub(crate) struct DenyScanner {
    pub(crate) automaton: AhoCorasick,
    /// Parallel to the automaton's pattern ids: `(value, rule)`.
    pub(crate) entries: Vec<(String, DenyRule)>,
}

impl DenyScanner {
    /// Build a scanner for the given deny-list entries. Returns
    /// `None` for an empty map (no automaton to compile).
    pub(crate) fn build(map: &HashMap<String, DenyRule>) -> Option<Self> {
        if map.is_empty() {
            return None;
        }
        // Sort for deterministic pattern-id ordering.
        let mut entries: Vec<(String, DenyRule)> =
            map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        let values: Vec<&str> = entries.iter().map(|(v, _)| v.as_str()).collect();
        let automaton = AhoCorasick::new(&values).expect("deny list values must compile");
        Some(Self { automaton, entries })
    }
}

impl fmt::Debug for DenyScanner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DenyScanner")
            .field("entries", &self.entries.len())
            .finish()
    }
}
