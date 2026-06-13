//! Entity selection criteria for policy rules.

use std::sync::LazyLock;

use hipstr::HipStr;
use nvisy_core::entity::{Entity, EntityLabelCatalog, EntityLabelRef};
use nvisy_core::primitive::ConfidenceThreshold;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::modality::DocumentModality;

/// Workspace-shipped built-in catalog. Selectors use this when no
/// custom catalog is plumbed in (selectors don't carry a catalog
/// today; tag matching dereferences against this catalog only).
static BUILTIN_CATALOG: LazyLock<EntityLabelCatalog> =
    LazyLock::new(EntityLabelCatalog::with_builtins);

/// Criteria for selecting which entities a policy rule applies to.
///
/// All fields use "empty means all" semantics: an empty `labels`
/// list matches every label, an empty `tags` list matches every
/// tag, and so on. When multiple fields are set, they are combined
/// with AND logic.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EntitySelector {
    /// Specific entity labels this selector matches. Empty means
    /// all labels.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<EntityLabelRef>,
    /// Tags this selector matches. An entity matches when its
    /// label (looked up in the workspace built-in catalog) carries
    /// any of the listed tags. Custom labels not registered in the
    /// catalog never match a tag selector — they must be matched
    /// by name via [`labels`].
    ///
    /// [`labels`]: Self::labels
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<String>")]
    pub tags: Vec<HipStr<'static>>,
    /// Minimum detection confidence required. Entities below this
    /// threshold are not matched. `None` means no threshold
    /// (matches any confidence).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence_threshold: Option<ConfidenceThreshold>,
}

impl EntitySelector {
    /// Create a selector that matches all entities.
    pub fn all() -> Self {
        Self::default()
    }

    /// Returns `true` if the given entity matches this selector.
    pub fn matches<M: DocumentModality>(&self, entity: &Entity<M>) -> bool {
        if let Some(threshold) = self.confidence_threshold
            && !threshold.admits(entity.confidence)
        {
            return false;
        }
        if !self.tags.is_empty() {
            let Some(catalog_entry) = BUILTIN_CATALOG.lookup(entity.label.as_str()) else {
                // Custom label (not in the built-in catalog) — tags
                // are catalog-side metadata, so the tag filter
                // never matches a custom label.
                return false;
            };
            if !self.tags.iter().any(|t| catalog_entry.has_tag(t)) {
                return false;
            }
        }
        if !self.labels.is_empty() && !self.labels.contains(&entity.label) {
            return false;
        }
        true
    }
}
