//! Policy rule types.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use nvisy_core::fs::DocumentType;
use super::spec::RedactionSpec;

use nvisy_detection::EntitySelector;

/// Conditions that must be met for a [`PolicyRule`] to apply.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleCondition {
    /// Document formats this rule applies to. Empty means all formats.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub document_types: Vec<DocumentType>,
    /// User roles this rule applies to.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<String>,
    /// Labels that must be present on the document.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_labels: Vec<String>,
}

/// Classifies what a policy rule does when it matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum RuleKind {
    /// Apply a redaction to the matched entity.
    Redaction,
    /// Require human review before any action is taken.
    Review,
    /// Flag the entity without redacting (for reporting / alerting).
    Alert,
    /// Block processing of the entire document.
    Block,
    /// Suppress a detection (treat as false positive).
    Suppress,
}

/// A single rule within a redaction policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    /// Unique identifier for this rule.
    pub id: Uuid,
    /// What this rule does when it matches.
    pub kind: RuleKind,
    /// Which entities this rule applies to.
    pub selector: EntitySelector,
    /// Redaction specification to apply when this rule matches (relevant when `kind` is `Redaction`).
    pub spec: RedactionSpec,
    /// Template string for the replacement value (e.g. `"[REDACTED]"`).
    pub replacement_template: String,
    /// Evaluation priority (lower numbers are evaluated first).
    pub priority: i32,
    /// Additional conditions for this rule to apply.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conditions: Option<RuleCondition>,
}
