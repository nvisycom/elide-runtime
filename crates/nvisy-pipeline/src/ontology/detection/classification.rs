//! Sensitivity classification result.

use serde::{Deserialize, Serialize};

use super::Sensitivity;

/// Result of sensitivity classification over a set of detected entities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationResult {
    /// Sensitivity assessment (level + risk score).
    pub sensitivity: Sensitivity,
    /// Total number of entities considered.
    pub total_entities: usize,
}
