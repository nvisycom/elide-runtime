//! Sensitivity level and assessment types.

use serde::{Deserialize, Serialize};

/// Sensitivity classification assigned to a document or content region.
///
/// Drives downstream policy: rules can be scoped to specific sensitivity
/// levels via [`RuleCondition`](crate::policy::RuleCondition).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SensitivityLevel {
    /// No sensitive data detected or all data is publicly available.
    Public,
    /// Internal use only — not intended for external distribution.
    Internal,
    /// Contains sensitive data requiring access controls.
    Confidential,
    /// Highly sensitive — regulated data requiring strict controls.
    Restricted,
}

/// Combined sensitivity assessment for a content source.
///
/// Pairs a discrete [`SensitivityLevel`] with an optional continuous
/// re-identification risk score in `[0.0, 1.0]`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
pub struct Sensitivity {
    /// Discrete sensitivity classification.
    pub level: SensitivityLevel,
    /// Re-identification risk score in the range `[0.0, 1.0]`.
    ///
    /// Estimates the likelihood that a data subject could be re-identified
    /// from the entities remaining after redaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_score: Option<f64>,
}
