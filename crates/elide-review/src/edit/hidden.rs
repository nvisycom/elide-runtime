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
/// Empty by default: with no decision, anonymize removes exactly
/// the items analyze marked sensitive.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct HiddenContentEdits {
    /// Items to remove that would otherwise be kept.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub remove: Vec<HiddenChoice>,
    /// Items to keep that would otherwise be removed.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub keep: Vec<HiddenChoice>,
    /// Remove every item, sensitive or not, when set: the nuclear
    /// option.
    ///
    /// Cannot be combined with `keep`: "remove everything" and
    /// "keep this one" answer the same question about that item
    /// differently, so the set is rejected rather than either
    /// silently winning.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remove_all: Option<RemoveAll>,
}

impl HiddenContentEdits {
    /// Whether no decision is recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.remove.is_empty() && self.keep.is_empty() && self.remove_all.is_none()
    }

    /// How many decisions are recorded, remove-all counting as one.
    #[must_use]
    pub fn len(&self) -> usize {
        self.remove.len() + self.keep.len() + usize::from(self.remove_all.is_some())
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
#[serde(rename_all = "camelCase")]
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
    let mut decided: HashMap<Uuid, Decision> = HashMap::new();
    let choices = edits
        .remove
        .iter()
        .map(|choice| (choice, Decision::Remove))
        .chain(edits.keep.iter().map(|choice| (choice, Decision::Keep)));
    for (choice, decision) in choices {
        if !items.contains(choice.id) {
            return Err(EditError::NotHiddenContent {
                entity_id: choice.id,
            });
        }
        if edits.remove_all.is_some() && decision == Decision::Keep {
            return Err(EditError::hidden_contradiction(
                choice.id,
                "removeAll",
                decision.name(),
            ));
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
