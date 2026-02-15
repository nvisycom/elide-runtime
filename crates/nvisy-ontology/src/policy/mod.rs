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

use crate::redaction::RedactionSpec;

/// A named redaction policy containing an ordered set of rules.
///
/// Policies are pure configuration — they describe *what* to detect and
/// *how* to handle it, independent of any specific content source.
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

/// A collection of policies to apply during a pipeline run.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
pub struct Policies {
    /// The policies to evaluate, in order.
    pub policies: Vec<Policy>,
}
