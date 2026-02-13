//! Explainability metadata for data protection decisions.
//!
//! An [`Explanation`] records why an action was taken — which model, rule,
//! and confidence level were involved. Types that carry this metadata
//! implement the [`Explainable`] trait.

use semver::Version;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entity::{DetectionMethod, ModelInfo};

/// Types that carry explainability metadata.
pub trait Explainable {
    /// Why this action was taken.
    fn explanation(&self) -> Option<&Explanation>;
}

/// Structured explainability metadata for a data protection decision.
///
/// Records why an action was taken, which model and rule were involved,
/// and who reviewed it. Complements the freeform `details` field on [`Audit`](super::Audit).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
pub struct Explanation {
    /// Detection model that produced the decision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelInfo>,
    /// Identifier of the policy rule that triggered the action.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<Uuid>,
    /// Detection confidence score.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    /// Detection method used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detection_method: Option<DetectionMethod>,
    /// Human-readable reason for the action.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Version of the policy that was evaluated.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "jsonschema", schemars(with = "Option<String>"))]
    pub policy_version: Option<Version>,
    /// Identifier of the reviewer who approved/rejected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewer_id: Option<String>,
}
