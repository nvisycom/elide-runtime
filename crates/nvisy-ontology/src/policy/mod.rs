//! Policy types, rules, and governance structures.

mod retention;
mod rule;
mod selector;
mod strategy;
mod summary;

use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use self::retention::{Retention, RetentionPolicy, RetentionScope};
pub use self::rule::{PolicyRule, RuleAction, RuleCondition};
pub use self::selector::EntitySelector;
pub use self::strategy::{AudioStrategy, ImageStrategy, Strategy, TextStrategy};
pub use self::summary::RedactionEntry;

/// A named redaction policy containing an ordered set of rules.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Policy {
    /// Unique identifier for this policy.
    pub id: Uuid,
    /// Human-readable policy name.
    pub name: String,
    /// Policy version.
    #[schemars(with = "String")]
    pub version: Version,
    /// Description of the policy's purpose.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Parent policy identifier for inheritance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extends: Option<Uuid>,
    /// Ordered list of rules.
    pub rules: Vec<PolicyRule>,
}

/// A collection of policies to apply during a pipeline run.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Policies {
    /// The policies to evaluate, in order.
    pub policies: Vec<Policy>,
}
