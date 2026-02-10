use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::data::DataItem;
use crate::types::RedactionMethod;

/// A redaction decision for a detected entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Redaction {
    #[serde(flatten)]
    pub data: DataItem,
    pub entity_id: Uuid,
    pub method: RedactionMethod,
    pub replacement_value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_rule_id: Option<String>,
    pub applied: bool,
}

impl Redaction {
    pub fn new(
        entity_id: Uuid,
        method: RedactionMethod,
        replacement_value: impl Into<String>,
    ) -> Self {
        Self {
            data: DataItem::new(),
            entity_id,
            method,
            replacement_value: replacement_value.into(),
            original_value: None,
            policy_rule_id: None,
            applied: false,
        }
    }

    pub fn with_original_value(mut self, value: impl Into<String>) -> Self {
        self.original_value = Some(value.into());
        self
    }

    pub fn with_policy_rule_id(mut self, id: impl Into<String>) -> Self {
        self.policy_rule_id = Some(id.into());
        self
    }
}
