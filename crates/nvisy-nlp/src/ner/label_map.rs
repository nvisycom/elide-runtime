//! [`LabelMap`]: translation from a backend-specific label string to
//! a [`LabelMapEntry`] (a `(category, kind)` pair from the ontology).
//!
//! Different NER backends use different label vocabularies:
//!
//! - BIO-tagged transformer models (e.g. `dslim/bert-base-NER` via
//!   [`OrtBackend`]) emit `B-PER`/`I-PER`/`B-ORG`/...; the [`LabelMap`]
//!   key is the BIO **base** (`"PER"`, `"ORG"`).
//! - Zero-shot models like GLiNER (via [`GlinerBackend`]) emit the
//!   raw label strings the caller asked about (`"person"`,
//!   `"organization"`).
//!
//! Both shapes flow through the same `String → LabelMapEntry` lookup.
//! This newtype centralises that and gives it a name so consumers
//! don't pass a bare `HashMap` around.
//!
//! Tokens that don't appear in the map are dropped at recognition
//! time, even if the model returned spans for them.
//!
//! `LabelMapEntry` is also the serde shape preset manifests use for
//! their JSON `label_map` field, so the same type round-trips between
//! disk and backend without conversion.
//!
//! [`OrtBackend`]: super::OrtBackend
//! [`GlinerBackend`]: super::GlinerBackend

use std::collections::HashMap;

use nvisy_ontology::entity::{EntityCategory, EntityKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `(category, kind)` pair attached to a label.
///
/// Serialised as `{"category": "personal_identity", "kind":
/// "person_name"}` — named fields rather than a tuple because that
/// is how preset manifests express the mapping in JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LabelMapEntry {
    /// Broad bucket the entity belongs to.
    pub category: EntityCategory,
    /// Specific entity kind.
    pub kind: EntityKind,
}

impl LabelMapEntry {
    /// Inline constructor for call sites that already have both
    /// halves.
    pub fn new(category: EntityCategory, kind: EntityKind) -> Self {
        Self { category, kind }
    }
}

/// Translation from a backend-specific label string to a
/// [`LabelMapEntry`].
///
/// Serialises as the inner `HashMap` (via `#[serde(transparent)]`), so
/// preset manifests can carry `label_map: LabelMap` directly without
/// declaring an intermediate alias.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema
)]
#[serde(transparent)]
pub struct LabelMap(HashMap<String, LabelMapEntry>);

impl LabelMap {
    /// Construct an empty map. Construction-time validation (e.g.
    /// "reserved label can't appear") is the caller's responsibility.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace a `label → (category, kind)` entry. Returns
    /// the previous entry if any.
    pub fn insert(
        &mut self,
        label: impl Into<String>,
        category: EntityCategory,
        kind: EntityKind,
    ) -> Option<LabelMapEntry> {
        self.0
            .insert(label.into(), LabelMapEntry::new(category, kind))
    }

    /// Look up the entry for `label`. `None` means the backend
    /// emitted a label the operator chose not to map — recognition
    /// should drop it.
    pub fn classify(&self, label: &str) -> Option<LabelMapEntry> {
        self.0.get(label).copied()
    }

    /// Whether `label` is in the map. Equivalent to
    /// [`classify`]`(label).is_some()` but doesn't allocate a copy of
    /// the entry.
    ///
    /// [`classify`]: Self::classify
    pub fn contains(&self, label: &str) -> bool {
        self.0.contains_key(label)
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the map has no entries.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Iterate over every label in the map. Order is unspecified.
    pub fn labels(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }

    /// Iterate over `(label, entry)` pairs. Order is unspecified.
    pub fn iter(&self) -> impl Iterator<Item = (&str, LabelMapEntry)> {
        self.0.iter().map(|(l, e)| (l.as_str(), *e))
    }
}
