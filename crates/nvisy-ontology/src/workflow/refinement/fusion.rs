//! Fusion node configuration and strategy.

use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::entity::RecognitionMethod;

/// Strategy for combining confidence scores from multiple detectors.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum FusionStrategy {
    /// Take the maximum confidence across all detectors.
    #[default]
    MaxConfidence,
    /// Weighted average by recognition method.
    WeightedAverage {
        /// Per-method weight (missing methods default to 1.0).
        weights: HashMap<RecognitionMethod, f64>,
    },
    /// Noisy-OR: `P = 1 − ∏(1 − pᵢ)` for independent detectors.
    NoisyOr,
}

/// Configuration for the `Fusion` graph node.
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Fusion {
    /// Remove overlapping duplicate entities before fusion.
    #[serde(default)]
    pub entity_deduplication: bool,
    /// Strategy for combining confidence scores.
    #[serde(default)]
    pub strategy: FusionStrategy,
    /// Adjust raw model scores to align with empirical precision targets.
    #[serde(default)]
    pub confidence_calibration: bool,
    /// Use surrounding document context to upgrade or downgrade confidence.
    #[serde(default)]
    pub contextual_adjustment: bool,
}
