//! Policy rule types.
//!
//! A [`PolicyRule`] defines when and how a specific redaction is applied,
//! based on entity categories, types, and confidence thresholds.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::detection::SensitivityLevel;
use crate::entity::{DocumentType, EntitySelector};
use crate::redaction::RedactionSpec;

/// Conditions that must be met for a [`PolicyRule`] to apply.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
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
    /// Sensitivity levels this rule applies to. Empty means all levels.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sensitivity_levels: Vec<SensitivityLevel>,
}

/// Classifies what a policy rule does when it matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
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

/// A single rule within a redaction [`Policy`](super::Policy).
///
/// Rules specify which entity categories and types they match, the minimum
/// confidence threshold, and the action to take. Rules are evaluated in
/// ascending priority order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
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
