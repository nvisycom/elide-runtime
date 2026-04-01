//! Policy types, rules, and governance structures.

mod retention;
mod rule;
mod selector;
mod strategy;

use derive_builder::Builder;
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use self::retention::{Retention, RetentionPolicy, RetentionScope};
pub use self::rule::{PolicyRule, RuleAction, RuleCondition};
pub use self::selector::EntitySelector;
pub use self::strategy::{AudioStrategy, ImageStrategy, Strategy, TextStrategy};

/// A named redaction policy containing an ordered set of rules.
#[derive(Debug, Clone, Builder, Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "PolicyBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
pub struct Policy {
    /// Unique identifier for this policy.
    #[builder(default = "Uuid::now_v7()")]
    pub id: Uuid,
    /// Human-readable policy name.
    pub name: String,
    /// Policy version.
    #[schemars(with = "String")]
    pub version: Version,
    /// Description of the policy's purpose.
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Default redaction strategy for entities that match no rule but
    /// exceed the confidence threshold. If `None`, unmatched entities
    /// are left unredacted.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_strategy: Option<Strategy>,
    /// Ordered list of rules.
    #[builder(default)]
    pub rules: Vec<PolicyRule>,
    /// Data retention policies governing how long each class of data
    /// (original content, redacted output, audit logs) is kept.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retention: Vec<RetentionPolicy>,
}

impl Policy {
    /// Start building a new policy.
    pub fn builder() -> PolicyBuilder {
        PolicyBuilder::default()
    }
}

/// A collection of policies to apply during a pipeline run.
///
/// Provides convenience methods for accessing flattened rules and
/// retention policies across all contained policies.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Policies {
    /// The policies to evaluate, in order.
    pub policies: Vec<Policy>,
}

impl Policies {
    /// All rules across all policies, sorted by priority (lower = higher precedence).
    pub fn all_rules(&self) -> Vec<&PolicyRule> {
        let mut rules: Vec<&PolicyRule> =
            self.policies.iter().flat_map(|p| p.rules.iter()).collect();
        rules.sort_by_key(|r| r.priority());
        rules
    }

    /// All retention policies across all policies.
    pub fn all_retention(&self) -> Vec<&RetentionPolicy> {
        self.policies
            .iter()
            .flat_map(|p| p.retention.iter())
            .collect()
    }

    /// The first default strategy found across policies, if any.
    pub fn default_strategy(&self) -> Option<&Strategy> {
        self.policies
            .iter()
            .find_map(|p| p.default_strategy.as_ref())
    }

    /// Look up the effective retention for a given scope.
    ///
    /// Returns the first matching retention policy found. If no policy
    /// specifies retention for the scope, returns `None` (meaning
    /// indefinite retention).
    pub fn retention_for(&self, scope: RetentionScope) -> Option<&Retention> {
        self.all_retention()
            .into_iter()
            .find(|rp| rp.scope == scope)
            .map(|rp| &rp.retention)
    }
}
