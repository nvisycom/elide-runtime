//! Sensitivity classification result.

use serde::{Deserialize, Serialize};

use super::SensitivityLevel;

/// Result of sensitivity classification over a set of detected entities.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
pub struct ClassificationResult {
    /// The computed sensitivity level.
    pub sensitivity_level: SensitivityLevel,
    /// Total number of entities considered.
    pub total_entities: usize,
    /// Re-identification risk score in the range `[0.0, 1.0]`, if computed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_score: Option<f64>,
}
