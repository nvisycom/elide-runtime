//! [`EntityLabelCatalog`] — name-indexed lookup over a set of
//! [`EntityLabel`]s.
//!
//! Constructed at runtime configuration time. Recognizers'
//! supported labels and selectors' tag-matching path both walk a
//! `EntityLabelCatalog`. The workspace ships a built-in catalog through
//! [`EntityLabelCatalog::with_builtins`]; deployments can register their own
//! labels alongside or instead of the built-ins via
//! [`EntityLabelCatalog::with_label`] / [`EntityLabelCatalog::with_labels`].

use std::collections::HashMap;
use std::sync::LazyLock;

use hipstr::HipStr;

use super::builtins::BUILT_INS;
use super::entity_label::EntityLabel;

/// Name-indexed catalog of [`EntityLabel`]s.
///
/// Built from a list of labels (mixing workspace-shipped built-ins
/// with deployment-defined custom labels). Construction copies each
/// label into a [`HashMap`] keyed by `HipStr` clone of the label's
/// name; subsequent lookups are O(1).
#[derive(Debug, Clone, Default)]
pub struct EntityLabelCatalog {
    by_name: HashMap<HipStr<'static>, EntityLabel>,
}

impl EntityLabelCatalog {
    /// Empty catalog. Built-ins must be registered explicitly via
    /// [`Self::with_label`] / [`Self::with_labels`]; use
    /// [`Self::with_builtins`] for the workspace-shipped set.
    pub fn new() -> Self {
        Self::default()
    }

    /// EntityLabelCatalog pre-populated with every workspace-shipped built-in
    /// label.
    pub fn with_builtins() -> Self {
        let mut cat = Self::new();
        for lazy in BUILT_INS {
            cat.insert(LazyLock::force(lazy).clone());
        }
        cat
    }

    /// Register a single label. Replaces any prior entry sharing
    /// the same [`EntityLabel::name`].
    pub fn insert(&mut self, label: EntityLabel) {
        self.by_name.insert(label.name.clone(), label);
    }

    /// Builder-style sibling of [`Self::insert`] returning `Self`.
    #[must_use]
    pub fn with_label(mut self, label: EntityLabel) -> Self {
        self.insert(label);
        self
    }

    /// Bulk-register a sequence of labels.
    #[must_use]
    pub fn with_labels<I>(mut self, labels: I) -> Self
    where
        I: IntoIterator<Item = EntityLabel>,
    {
        for l in labels {
            self.insert(l);
        }
        self
    }

    /// Look up a label by name. Returns `None` for names not
    /// registered in this catalog.
    pub fn lookup(&self, name: &str) -> Option<&EntityLabel> {
        self.by_name.get(name)
    }

    /// Iterator over every registered label, in no particular order.
    pub fn iter(&self) -> impl Iterator<Item = &EntityLabel> + '_ {
        self.by_name.values()
    }

    /// Number of labels in the catalog.
    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    /// `true` when the catalog is empty.
    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_catalog_resolves_known_names() {
        let cat = EntityLabelCatalog::with_builtins();
        let l = cat.lookup("payment_card").expect("built-in");
        assert!(l.has_tag("financial"));
        assert!(cat.lookup("acme_internal_id").is_none());
    }

    #[test]
    fn catalog_accepts_custom_labels_alongside_builtins() {
        let custom = EntityLabel::new("acme_internal_id").with_tags(["custom"]);
        let cat = EntityLabelCatalog::with_builtins().with_label(custom);
        assert!(cat.lookup("payment_card").is_some());
        let acme = cat.lookup("acme_internal_id").expect("custom registered");
        assert!(acme.has_tag("custom"));
    }
}
