//! Redaction policies and rules.

use serde::{Deserialize, Serialize};
use crate::datatypes::Data;
use crate::ontology::entity::EntityCategory;
use crate::ontology::redaction::RedactionMethod;

/// A single rule within a redaction [`Policy`].
///
/// Rules specify which entity categories and types they match, the minimum
/// confidence threshold, and the redaction method to apply. Rules are
/// evaluated in ascending priority order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PolicyRule {
    /// Unique identifier for this rule within its policy.
    pub id: String,
    /// Human-readable name for display purposes.
    pub name: String,
    /// Entity categories this rule applies to. Empty means all categories.
    pub categories: Vec<EntityCategory>,
    /// Specific entity type names this rule applies to. Empty means all types.
    pub entity_types: Vec<String>,
    /// Minimum detection confidence required for this rule to trigger.
    pub confidence_threshold: f64,
    /// Redaction strategy to apply when this rule matches.
    pub method: RedactionMethod,
    /// Template string for the replacement value (e.g. `"[REDACTED]"`).
    pub replacement_template: String,
    /// Whether this rule is active. Disabled rules are skipped during evaluation.
    pub enabled: bool,
    /// Evaluation priority (lower numbers are evaluated first).
    pub priority: i32,
}

/// A named redaction policy containing an ordered set of rules.
///
/// Policies are evaluated by [`find_matching_rule`](Policy::find_matching_rule)
/// which returns the first matching enabled rule sorted by priority.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Policy {
    /// Common data-item fields (id, parent_id, metadata).
    #[serde(flatten)]
    pub data: Data,
    /// Human-readable policy name.
    pub name: String,
    /// Ordered list of redaction rules.
    pub rules: Vec<PolicyRule>,
    /// Fallback redaction method when no rule matches.
    pub default_method: RedactionMethod,
    /// Fallback confidence threshold when no rule matches.
    pub default_confidence_threshold: f64,
}

impl Policy {
    /// Create a new policy with the given name and rules, using default
    /// fallback method ([`Mask`](RedactionMethod::Mask)) and threshold (0.5).
    pub fn new(name: impl Into<String>, rules: Vec<PolicyRule>) -> Self {
        Self {
            data: Data::new(),
            name: name.into(),
            rules,
            default_method: RedactionMethod::Mask,
            default_confidence_threshold: 0.5,
        }
    }

    /// Override the fallback redaction method.
    pub fn with_default_method(mut self, method: RedactionMethod) -> Self {
        self.default_method = method;
        self
    }

    /// Override the fallback confidence threshold.
    pub fn with_default_confidence_threshold(mut self, threshold: f64) -> Self {
        self.default_confidence_threshold = threshold;
        self
    }

    /// Find the first matching enabled rule for a given entity.
    ///
    /// Rules are sorted by priority (ascending). A rule matches when:
    /// - It is enabled
    /// - The entity's confidence meets the rule's threshold
    /// - The entity's category is in the rule's categories (or categories is empty)
    /// - The entity's type is in the rule's entityTypes (or entityTypes is empty)
    pub fn find_matching_rule(
        &self,
        category: EntityCategory,
        entity_type: &str,
        confidence: f64,
    ) -> Option<&PolicyRule> {
        let mut sorted: Vec<&PolicyRule> = self.rules.iter().collect();
        sorted.sort_by_key(|r| r.priority);

        for rule in sorted {
            if !rule.enabled {
                continue;
            }
            if confidence < rule.confidence_threshold {
                continue;
            }
            if !rule.categories.is_empty() && !rule.categories.contains(&category) {
                continue;
            }
            if !rule.entity_types.is_empty()
                && !rule.entity_types.iter().any(|t| t == entity_type)
            {
                continue;
            }
            return Some(rule);
        }

        None
    }
}
