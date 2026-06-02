//! [`ContextRegistry`]: the `name → Context` lookup the enhancer
//! reads at boost time.
//!
//! The recognizer side of the pipeline (`PatternRecognizer`,
//! `NlpRecognizer`, `GlinerRecognizer`, …) registers one entry per
//! source name — for patterns that's one entry per
//! `Regex`/`Dictionary` rule; for NER it's typically one entry per
//! recognizer keyed on the recognizer's name. The enhancer reads
//! the entity's first-step provenance, pulls the name, and looks
//! up the [`Context`] here.
//!
//! Last-write-wins on duplicate names: callers are responsible for
//! choosing distinct keys when mixing per-rule and per-recognizer
//! registrations.

use std::collections::HashMap;

use super::Context;

/// Lookup table the [`ContextEnhancer`](super::ContextEnhancer)
/// reads at boost time.
///
/// Construct with [`new`](Self::new), populate with
/// [`with_entry`](Self::with_entry) /
/// [`with_entries`](Self::with_entries), then hand to a
/// [`ContextEnhancerBuilder`](super::ContextEnhancerBuilder).
#[derive(Debug, Clone, Default)]
pub struct ContextRegistry {
    entries: HashMap<String, Context>,
}

impl ContextRegistry {
    /// Empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register one entry. Last write wins on duplicate names.
    #[must_use]
    pub fn with_entry(mut self, name: impl Into<String>, context: Context) -> Self {
        let context_name = name.into();
        if !context.is_empty() {
            self.entries.insert(context_name, context);
        }
        self
    }

    /// Register many entries.
    #[must_use]
    pub fn with_entries<I, S>(mut self, entries: I) -> Self
    where
        I: IntoIterator<Item = (S, Context)>,
        S: Into<String>,
    {
        for (name, context) in entries {
            let context_name = name.into();
            if !context.is_empty() {
                self.entries.insert(context_name, context);
            }
        }
        self
    }

    /// Look up the [`Context`] for `name`. Returns `None` when the
    /// name was never registered or when the registered context
    /// had an empty keyword list (which is treated as "not
    /// registered" — see [`with_entry`](Self::with_entry)).
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Context> {
        self.entries.get(name)
    }

    /// Number of registered names with non-empty contexts.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the registry has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Extend<(String, Context)> for ContextRegistry {
    fn extend<I: IntoIterator<Item = (String, Context)>>(&mut self, iter: I) {
        for (name, context) in iter {
            if !context.is_empty() {
                self.entries.insert(name, context);
            }
        }
    }
}

impl FromIterator<(String, Context)> for ContextRegistry {
    fn from_iter<I: IntoIterator<Item = (String, Context)>>(iter: I) -> Self {
        let mut registry = Self::new();
        registry.extend(iter);
        registry
    }
}
