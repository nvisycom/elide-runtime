//! Redaction methods and records.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use nvisy_core::datatypes::Data;

/// Strategy used to redact or obfuscate a detected entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum RedactionMethod {
    /// Replace characters with a mask character (e.g. `***-**-1234`).
    Mask,
    /// Substitute with a fixed placeholder string.
    Replace,
    /// Replace with a one-way hash of the original value.
    Hash,
    /// Encrypt the value so it can be recovered later with a key.
    Encrypt,
    /// Remove the value entirely from the output.
    Remove,
    /// Blur a region in an image.
    Blur,
    /// Overlay an opaque block over a region in an image.
    Block,
    /// Replace with a synthetically generated realistic value.
    Synthesize,
}

/// A redaction decision recording how a specific entity was (or will be) redacted.
///
/// Each `Redaction` is linked to exactly one [`Entity`](super::entity::Entity)
/// via `entity_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Redaction {
    /// Common data-item fields (id, parent_id, metadata).
    #[serde(flatten)]
    pub data: Data,
    /// Identifier of the entity being redacted.
    pub entity_id: Uuid,
    /// Redaction strategy applied to the entity.
    pub method: RedactionMethod,
    /// The string that replaces the original value in the output.
    pub replacement_value: String,
    /// The original sensitive value, retained for audit purposes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_value: Option<String>,
    /// Identifier of the policy rule that triggered this redaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_rule_id: Option<String>,
    /// Whether the redaction has been applied to the output content.
    pub applied: bool,
}

impl Redaction {
    /// Create a new pending redaction for the given entity.
    pub fn new(
        entity_id: Uuid,
        method: RedactionMethod,
        replacement_value: impl Into<String>,
    ) -> Self {
        Self {
            data: Data::new(),
            entity_id,
            method,
            replacement_value: replacement_value.into(),
            original_value: None,
            policy_rule_id: None,
            applied: false,
        }
    }

    /// Record the original sensitive value for audit trail purposes.
    pub fn with_original_value(mut self, value: impl Into<String>) -> Self {
        self.original_value = Some(value.into());
        self
    }

    /// Associate this redaction with the policy rule that triggered it.
    pub fn with_policy_rule_id(mut self, id: impl Into<String>) -> Self {
        self.policy_rule_id = Some(id.into());
        self
    }
}
