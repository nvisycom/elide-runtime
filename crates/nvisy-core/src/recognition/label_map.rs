//! [`LabelMap`]: backend label → canonical label-name translation.
//!
//! Shared translation table used by every model-driven recognizer
//! (NER backends, LLM recognizers, …). Lets a recognizer consume
//! raw model labels uniformly regardless of which backend produced
//! them — swap backends without re-implementing translation.
//!
//! The map is bidirectional in spirit (look up an entity label
//! name to find the canonical model label a backend should be asked
//! for) but the primary path is model-label → entity-label-name.
//! The reverse lookup is a linear scan.

use std::borrow::Cow;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::entity::{EntityLabelCatalog, EntityLabelRef};

/// Translation table from raw model labels to entity label names.
///
/// The default ([`LabelMap::canonical`]) maps every name in a
/// [`EntityLabelCatalog`] to itself, so backends that already return canonical
/// names pass through unchanged. Custom backends register their
/// own model-specific labels via [`with_entry`].
///
/// [`with_entry`]: Self::with_entry
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LabelMap {
    entries: HashMap<String, EntityLabelRef>,
}

impl LabelMap {
    /// Empty map. Backends with no recognizable labels see every
    /// span dropped — typically you want [`Self::canonical`] /
    /// [`Self::canonical_from`] instead.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Identity map from every label name in the workspace
    /// built-in [`EntityLabelCatalog`] to itself. Convenience wrapper around
    /// [`Self::canonical_from`] for callers that don't need custom
    /// labels.
    #[must_use]
    pub fn canonical() -> Self {
        Self::canonical_from(&EntityLabelCatalog::with_builtins())
    }

    /// Identity map over every name in the supplied catalog.
    /// Backends that already return canonical names — or that
    /// have been pre-registered with the catalog — pass through
    /// unchanged.
    #[must_use]
    pub fn canonical_from(catalog: &EntityLabelCatalog) -> Self {
        let entries = catalog
            .iter()
            .map(|label| {
                (
                    label.name.to_string(),
                    EntityLabelRef::from(label.name.clone()),
                )
            })
            .collect();
        Self { entries }
    }

    /// Register one model-label → entity-label-ref entry. Last
    /// write wins on duplicates.
    #[must_use]
    pub fn with_entry(
        mut self,
        model_label: impl Into<Cow<'static, str>>,
        entity_label: impl Into<EntityLabelRef>,
    ) -> Self {
        self.entries
            .insert(model_label.into().into_owned(), entity_label.into());
        self
    }

    /// Register many entries.
    #[must_use]
    pub fn with_entries<I, K, V>(mut self, entries: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<EntityLabelRef>,
    {
        for (model_label, entity_label) in entries {
            self.entries.insert(model_label.into(), entity_label.into());
        }
        self
    }

    /// Look up a raw model label. `None` when not registered.
    #[must_use]
    pub fn lookup(&self, model_label: &str) -> Option<&EntityLabelRef> {
        self.entries.get(model_label)
    }

    /// Find a model label that maps to the given entity label
    /// name. Linear scan; returns the first match. Used by
    /// zero-shot backends that need to format requested-labels as
    /// raw model labels for the service.
    #[must_use]
    pub fn model_label_for(&self, entity_label: &str) -> Option<&str> {
        self.entries
            .iter()
            .find_map(|(m, e)| (e.as_str() == entity_label).then_some(m.as_str()))
    }

    /// Number of registered entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the map is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::builtins;

    #[test]
    fn canonical_map_resolves_known_labels() {
        let map = LabelMap::canonical();
        assert_eq!(
            map.lookup("email_address").map(|r| r.as_str()),
            Some("email_address")
        );
        assert!(map.lookup("ssn").is_none());
    }

    #[test]
    fn custom_entries_override_canonical() {
        let map = LabelMap::canonical().with_entry(
            "PER",
            EntityLabelRef::from(builtins::PERSON_NAME.name.clone()),
        );
        assert_eq!(map.lookup("PER").map(|r| r.as_str()), Some("person_name"));
    }

    #[test]
    fn model_label_for_round_trips() {
        let map = LabelMap::canonical();
        assert_eq!(map.model_label_for("email_address"), Some("email_address"));
    }
}
