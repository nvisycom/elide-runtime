//! [`HiddenContentEdits`]: what a reviewer decides about hidden
//! content.
//!
//! Analyze makes every metadata field and collaboration item
//! (comment, comment author, tracked change) an entity, and marks
//! the sensitive ones: a label the request declares, or a finding
//! in the value. Anonymize removes those and keeps the rest. A
//! reviewer changes that by entity id, whatever the item's
//! modality:
//!
//! - **remove** an item that would be kept,
//! - **keep** an item that would be removed,
//! - **remove all**, the nuclear option: every item goes.
//!
//! Item-by-item decisions and a remove-all are two shapes, not a
//! flag beside a list: a remove-all decides every item, so a set
//! cannot also keep one.
//!
//! Each lands as a `Manual` event on the item, so who decided and
//! why is auditable: a remove (and each item a remove-all reaches)
//! as a `Confirm`, and a keep as a `Suppress` of everything within
//! the item, which elide's redaction pass skips. The latest verdict
//! on the item wins.

use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::error::EditError;
use crate::index::HiddenItems;

/// A reviewer's decisions about one document's hidden content.
///
/// On the wire either `{"choose": {"remove": [...], "keep": [...]}}`
/// or `{"removeAll": {...}}`, never both: removing every item and
/// keeping one answer the same question differently.
///
/// Empty by default: with no decision, anonymize removes exactly
/// the items analyze marked sensitive.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum HiddenContentEdits {
    /// Item-by-item decisions.
    Choose(HiddenChoices),
    /// Remove every item, sensitive or not: the nuclear option.
    RemoveAll(RemoveAll),
}

impl Default for HiddenContentEdits {
    fn default() -> Self {
        Self::Choose(HiddenChoices::default())
    }
}

impl HiddenContentEdits {
    /// Whether no decision is recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Choose(choices) => choices.remove.is_empty() && choices.keep.is_empty(),
            Self::RemoveAll(_) => false,
        }
    }

    /// How many decisions are recorded, remove-all counting as one.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Choose(choices) => choices.remove.len() + choices.keep.len(),
            Self::RemoveAll(_) => 1,
        }
    }
}

impl From<HiddenChoices> for HiddenContentEdits {
    fn from(choices: HiddenChoices) -> Self {
        Self::Choose(choices)
    }
}

impl From<RemoveAll> for HiddenContentEdits {
    fn from(remove_all: RemoveAll) -> Self {
        Self::RemoveAll(remove_all)
    }
}

/// Item-by-item decisions about hidden content.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct HiddenChoices {
    /// Items to remove that would otherwise be kept.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub remove: Vec<HiddenChoice>,
    /// Items to keep that would otherwise be removed.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub keep: Vec<HiddenChoice>,
}

impl HiddenChoices {
    /// Also remove the item `choice` names.
    #[must_use]
    pub fn remove(mut self, choice: HiddenChoice) -> Self {
        self.remove.push(choice);
        self
    }

    /// Also keep the item `choice` names.
    #[must_use]
    pub fn keep(mut self, choice: HiddenChoice) -> Self {
        self.keep.push(choice);
        self
    }
}

/// One hidden-content item a reviewer decided about.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HiddenChoice {
    /// The item's entity id, as analyze reported it.
    pub id: Uuid,
    /// Why the reviewer decided so, when they said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl HiddenChoice {
    /// A decision about the item `id`, with no reason given.
    #[must_use]
    pub fn new(id: Uuid) -> Self {
        Self { id, reason: None }
    }

    /// Give the reason for the decision.
    #[must_use]
    pub fn because(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

/// A reviewer's decision to remove every hidden-content item.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoveAll {
    /// Why the reviewer decided so, when they said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl RemoveAll {
    /// Remove every item, giving `reason`.
    #[must_use]
    pub fn because(reason: impl Into<String>) -> Self {
        Self {
            reason: Some(reason.into()),
        }
    }
}

/// What a reviewer decided about one item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Decision {
    Remove,
    Keep,
}

impl Decision {
    /// The field it was sent in, for an error naming the pair.
    const fn name(self) -> &'static str {
        match self {
            Self::Remove => "remove",
            Self::Keep => "keep",
        }
    }
}

/// Reject decisions this report cannot take: an id naming no
/// hidden-content item, or two decisions answering the same item
/// differently.
pub(crate) fn validate(edits: &HiddenContentEdits, items: &HiddenItems) -> Result<(), EditError> {
    let HiddenContentEdits::Choose(choices) = edits else {
        return Ok(());
    };
    let mut decided: HashMap<Uuid, Decision> = HashMap::new();
    let all = choices
        .remove
        .iter()
        .map(|choice| (choice, Decision::Remove))
        .chain(choices.keep.iter().map(|choice| (choice, Decision::Keep)));
    for (choice, decision) in all {
        if !items.contains(choice.id) {
            return Err(EditError::NotHiddenContent {
                entity_id: choice.id,
            });
        }
        if let Some(earlier) = decided.insert(choice.id, decision)
            && earlier != decision
        {
            return Err(EditError::hidden_contradiction(
                choice.id,
                earlier.name(),
                decision.name(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::HiddenContentEdits;

    fn read(value: serde_json::Value) -> Result<HiddenContentEdits, serde_json::Error> {
        serde_json::from_value(value)
    }

    /// The schema is a choice between two closed shapes, so it
    /// refuses what reading refuses.
    #[test]
    fn the_schema_is_one_of_two_closed_shapes() {
        let schema = schemars::schema_for!(HiddenContentEdits).to_value();
        let arms = schema["oneOf"]
            .as_array()
            .unwrap_or_else(|| panic!("a choice: {schema:#}"));
        assert_eq!(arms.len(), 2, "{schema:#}");
        for arm in arms {
            assert_eq!(arm["additionalProperties"], false, "{arm:#}");
        }
    }

    #[test]
    fn a_set_mixing_the_shapes_or_carrying_null_is_refused() {
        let id = uuid::Uuid::now_v7();
        for bad in [
            json!({ "removeAll": {}, "choose": { "keep": [{ "id": id }] } }),
            json!({ "removeAll": null }),
            json!({ "choose": null }),
            json!({ "choose": { "remove": null } }),
            json!({ "choose": { "removeAll": {} } }),
            json!({ "remove": [] }),
        ] {
            read(bad.clone()).expect_err(&format!("{bad} is refused"));
        }
    }

    #[test]
    fn each_shape_reads_back() {
        let id = uuid::Uuid::now_v7();
        let all = read(json!({ "removeAll": { "reason": "public" } })).unwrap();
        assert!(matches!(all, HiddenContentEdits::RemoveAll(_)));
        let chosen = read(json!({ "choose": { "keep": [{ "id": id }] } })).unwrap();
        assert_eq!(chosen.len(), 1);
        let empty = read(json!({ "choose": {} })).unwrap();
        assert!(empty.is_empty());
    }
}
