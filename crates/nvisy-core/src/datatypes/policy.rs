use serde::{Deserialize, Serialize};
use crate::data::DataItem;
use crate::types::{EntityCategory, RedactionMethod};

/// A single rule within a redaction policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    pub id: String,
    pub name: String,
    pub categories: Vec<EntityCategory>,
    pub entity_types: Vec<String>,
    pub confidence_threshold: f64,
    pub method: RedactionMethod,
    pub replacement_template: String,
    pub enabled: bool,
    pub priority: i32,
}

/// A redaction policy containing rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    #[serde(flatten)]
    pub data: DataItem,
    pub name: String,
    pub rules: Vec<PolicyRule>,
    pub default_method: RedactionMethod,
    pub default_confidence_threshold: f64,
}

impl Policy {
    pub fn new(name: impl Into<String>, rules: Vec<PolicyRule>) -> Self {
        Self {
            data: DataItem::new(),
            name: name.into(),
            rules,
            default_method: RedactionMethod::Mask,
            default_confidence_threshold: 0.5,
        }
    }

    pub fn with_default_method(mut self, method: RedactionMethod) -> Self {
        self.default_method = method;
        self
    }

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
