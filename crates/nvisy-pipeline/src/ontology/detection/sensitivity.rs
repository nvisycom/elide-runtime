//! Sensitivity level and assessment types.

use serde::{Deserialize, Serialize};

/// Sensitivity classification assigned to a document or content region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Sensitivity {
    /// Discrete sensitivity classification.
    pub level: SensitivityLevel,
    /// Re-identification risk score in the range `[0.0, 1.0]`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_score: Option<f64>,
}
