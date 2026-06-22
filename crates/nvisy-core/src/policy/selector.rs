//! Entity selection criteria for policy rules.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Criteria for selecting which entities a policy rule applies to.
///
/// All fields use "empty means all" semantics: an empty `labels`
/// list matches every label, an empty `tags` list matches every
/// tag, and so on. When multiple fields are set, they are combined
/// with AND logic.
///
/// Selector evaluation (`does this entity match?`) happens at apply
/// time inside the engine — engine has both the matched entity and
/// the per-request label catalog in scope; nvisy-core only owns the
/// wire spec.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EntitySelector {
    /// Specific entity labels this selector matches. Empty means
    /// all labels.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    /// Tags this selector matches. An entity matches when its
    /// label — looked up in the per-request catalog — carries any
    /// of the listed tags. Labels not registered in the catalog
    /// never match a tag selector and must be matched by name via
    /// [`labels`].
    ///
    /// [`labels`]: Self::labels
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Minimum detection confidence required. Entities below this
    /// threshold are not matched. `None` means no threshold
    /// (matches any confidence).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence_threshold: Option<f32>,
}

impl EntitySelector {
    /// Create a selector that matches all entities.
    pub fn all() -> Self {
        Self::default()
    }
}
