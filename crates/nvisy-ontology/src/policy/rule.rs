//! Policy rule types.

use derive_builder::Builder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::selector::EntitySelector;
use super::strategy::Strategy;

/// Conditions that must be met for a [`PolicyRule`] to apply.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct RuleCondition {
    /// Labels that must be present on the document.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_labels: Vec<String>,
}

/// The action a policy rule performs when it matches an entity.
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
    /// Flag the entity without redacting (for reporting / alerting).
    Alert,
    /// Block processing of the entire document.
    Block,
    /// Suppress a detection (treat as false positive).
    Suppress,
}

/// A single rule within a policy.
#[derive(Debug, Clone, Builder, Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "PolicyRuleBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
pub struct PolicyRule {
    /// Unique identifier for this rule.
    #[builder(default = "Uuid::now_v7()")]
    pub id: Uuid,
    /// Which entities this rule applies to.
    pub selector: EntitySelector,
    /// What this rule does when it matches.
    pub action: RuleAction,
    /// Evaluation priority (lower numbers are evaluated first).
    #[builder(default, setter(into = false))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    /// Additional conditions for this rule to apply.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conditions: Option<RuleCondition>,
    /// Whether this rule is active. Disabled rules are skipped during
    /// evaluation without needing to remove them from the policy.
    #[builder(default = "true")]
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

impl PolicyRule {
    /// Start building a new policy rule.
    pub fn builder() -> PolicyRuleBuilder {
        PolicyRuleBuilder::default()
    }

    /// Evaluation priority (lower = higher precedence, default 0).
    pub fn priority(&self) -> i32 {
        self.priority.unwrap_or(0)
    }
}
