//! Strategy policy types for entity redaction.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::selector::EntitySelector;
use super::strategy::Strategy;

/// The action a strategy policy performs when it matches an entity.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
#[non_exhaustive]
pub enum RuleAction {
    /// Apply a redaction to the matched entity.
    Redact {
        /// Redaction strategy to apply.
        strategy: Strategy,
    },
    /// Require human review before any action is taken.
    Review,
    /// Flag the entity without redacting (for reporting/alerting).
    Alert,
    /// Block processing of the entire document.
    Block,
    /// Suppress a detection (treat as false positive).
    Suppress,
}

/// A condition that must be met for a strategy policy to apply.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum RuleCondition {
    /// All specified document labels must be present.
    Labels {
        /// Document labels that must all be present.
        labels: Vec<String>,
    },
}

/// A single entity redaction strategy: selector + action + conditions.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StrategyPolicy {
    /// Which entities this strategy applies to.
    pub selector: EntitySelector,
    /// What this strategy does when it matches.
    pub action: RuleAction,
    /// Evaluation priority (lower numbers are evaluated first).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    /// Conditions that must all be met for this strategy to apply.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<RuleCondition>,
    /// Whether this strategy is active.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

impl StrategyPolicy {
    /// Evaluation priority (lower = higher precedence, default 0).
    pub fn priority(&self) -> i32 {
        self.priority.unwrap_or(0)
    }
}
