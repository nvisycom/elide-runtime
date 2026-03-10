//! Policy rule types.

use nvisy_core::media::DocumentType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::selector::EntitySelector;
use super::strategy::Strategy;

/// Conditions that must be met for a [`PolicyRule`] to apply.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct RuleCondition {
    /// Document formats this rule applies to. Empty means all formats.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub document_types: Vec<DocumentType>,
    /// Labels that must be present on the document.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_labels: Vec<String>,
}

/// The action a policy rule performs when it matches an entity.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum RuleAction {
    /// Apply a redaction to the matched entity.
    Redact {
        /// Redaction strategy to apply.
        strategy: Strategy,
    },
    /// Require human review before any action is taken.
    Review,
    /// Flag the entity without redacting (for reporting / alerting).
    Alert,
    /// Block processing of the entire document.
    Block,
    /// Suppress a detection (treat as false positive).
    Suppress,
}

/// A single rule within a policy.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PolicyRule {
    /// Unique identifier for this rule.
    pub id: Uuid,
    /// Which entities this rule applies to.
    pub selector: EntitySelector,
    /// What this rule does when it matches.
    pub action: RuleAction,
    /// Evaluation priority (lower numbers are evaluated first).
    pub priority: i32,
    /// Additional conditions for this rule to apply.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conditions: Option<RuleCondition>,
}
