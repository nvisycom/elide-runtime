//! Redaction policies and rules.

use semver::Version;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::rule::PolicyRule;
use super::regulation::RegulationKind;
use nvisy_ontology::specification::RedactionInput;

/// A named redaction policy containing an ordered set of rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    /// Unique identifier for this policy.
    pub id: Uuid,
    /// Human-readable policy name.
    pub name: String,
    /// Policy version.
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
    pub default_spec: RedactionInput,
    /// Fallback confidence threshold when no rule matches.
    pub default_confidence_threshold: f64,
}

/// A collection of policies to apply during a pipeline run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policies {
    /// The policies to evaluate, in order.
    pub policies: Vec<Policy>,
}
