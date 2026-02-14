//! Redaction policies and rules.
//!
//! A [`Policy`] is a named, versioned set of [`PolicyRule`]s that govern
//! how detected entities are redacted. Policies may be associated with a
//! [`RegulationKind`] and support inheritance via the `extends` field.

mod evaluation;
mod regulation;
mod rule;

pub use evaluation::PolicyEvaluation;
pub use regulation::RegulationKind;
pub use rule::{PolicyRule, RuleCondition, RuleKind};

use semver::Version;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entity::EntityCategory;
use crate::redaction::{RedactionSpec, TextRedactionSpec};

/// A named redaction policy containing an ordered set of rules.
///
/// Policies are pure configuration — they describe *what* to detect and
/// *how* to handle it, independent of any specific content source.
///
/// Evaluated by [`find_matching_rule`](Policy::find_matching_rule)
/// which returns the first matching enabled rule sorted by priority.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
pub struct Policy {
    /// Unique identifier for this policy.
    pub id: Uuid,
    /// Human-readable policy name.
    pub name: String,
    /// Policy version.
    #[cfg_attr(feature = "jsonschema", schemars(with = "String"))]
    pub version: Version,
    /// Description of the policy's purpose.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Parent policy identifier for inheritance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extends: Option<Uuid>,
    /// Compliance regulation this policy targets.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regulation: Option<RegulationKind>,
    /// Ordered list of rules.
    pub rules: Vec<PolicyRule>,
    /// Fallback redaction specification when no rule matches.
    pub default_spec: RedactionSpec,
    /// Fallback confidence threshold when no rule matches.
    pub default_confidence_threshold: f64,
}

impl Policy {
    /// Create a new policy with the given name, version, and rules, using default
    /// fallback spec ([`TextRedactionSpec::Mask`]) and threshold (0.5).
    pub fn new(
        name: impl Into<String>,
        version: Version,
        rules: Vec<PolicyRule>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            version,
            description: None,
            extends: None,
            regulation: None,
            rules,
            default_spec: RedactionSpec::Text(TextRedactionSpec::Mask { mask_char: '*' }),
            default_confidence_threshold: 0.5,
        }
    }

    /// Override the fallback redaction specification.
    pub fn with_default_spec(mut self, spec: RedactionSpec) -> Self {
        self.default_spec = spec;
        self
    }

    /// Override the fallback confidence threshold.
    pub fn with_default_confidence_threshold(mut self, threshold: f64) -> Self {
        self.default_confidence_threshold = threshold;
        self
    }

    /// Find the first matching enabled rule for a given entity.
    ///
    /// Rules are sorted by priority (ascending). A rule matches when it is
    /// enabled and its [`EntitySelector`] matches the given entity properties.
    pub fn find_matching_rule(
        &self,
        category: &EntityCategory,
        entity_type: &str,
        confidence: f64,
    ) -> Option<&PolicyRule> {
        let mut sorted: Vec<&PolicyRule> = self.rules.iter().collect();
        sorted.sort_by_key(|r| r.priority);

        for rule in sorted {
            if !rule.enabled {
                continue;
            }
            if rule.selector.matches(category, entity_type, confidence) {
                return Some(rule);
            }
        }

        None
    }
}
