//! [`LabelSchema`]: wire shape for [`elide_core::entity::Label`].

use elide_core::entity::Label;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Wire-shape proxy for [`elide_core::entity::Label`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "Label")]
pub struct LabelSchema {
    /// Stable identifier, e.g. `"email_address"`.
    pub name: String,
    /// Optional human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Free-form tags policy selectors can target.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

impl From<LabelSchema> for Label {
    fn from(s: LabelSchema) -> Self {
        let label = match s.description {
            Some(desc) => Label::described(s.name, desc),
            None => Label::new(s.name),
        };
        if s.tags.is_empty() {
            label
        } else {
            label.with_tags(s.tags)
        }
    }
}

impl From<Label> for LabelSchema {
    fn from(l: Label) -> Self {
        Self {
            name: l.name().to_owned(),
            description: l.description().map(str::to_owned),
            tags: l.tags().iter().map(|t| t.as_str().to_owned()).collect(),
        }
    }
}
