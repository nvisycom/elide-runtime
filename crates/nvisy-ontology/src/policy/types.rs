//! Redaction policies and rules.

use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::rule::PolicyRule;

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
