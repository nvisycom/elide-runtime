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
    /// Parent policy identifier for inheritance.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extends: Option<Uuid>,
    /// Ordered list of rules.
    #[builder(default)]
    pub rules: Vec<PolicyRule>,
}

impl Policy {
    /// Start building a new policy.
    pub fn builder() -> PolicyBuilder {
        PolicyBuilder::default()
    }
}

/// A collection of policies to apply during a pipeline run.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Policies {
    /// The policies to evaluate, in order.
    pub policies: Vec<Policy>,
}
