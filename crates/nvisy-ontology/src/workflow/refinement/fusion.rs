//! Fusion node configuration and strategy.

use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::entity::RecognitionMethod;

/// How entity values and locations are matched when grouping.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum GroupingCriteria {
    /// Exact value match + overlapping location.
    #[default]
    Strict,
    /// Case-insensitive, trimmed value match + overlapping location.
    Normalized,
    /// Substring containment (shorter value is prefix/substring of longer)
    /// + overlapping location. Groups "John" with "John Smith".
    Narrowing,
    /// Same as narrowing but ignores location — groups the same entity
    /// across non-overlapping regions (e.g. cross-chunk fusion).
    Widening,
}

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

/// Per-method confidence multiplier applied before fusion.
///
/// Maps a [`RecognitionMethod`] to a scaling factor. Methods not present
/// in the map are left unchanged (implicit multiplier of 1.0).
pub type CalibrationMap = HashMap<RecognitionMethod, f64>;

/// Configuration for the `Fusion` graph node.
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Fusion {
    /// How to match entity values and locations when grouping.
    #[serde(default)]
    pub grouping: GroupingCriteria,
    /// Strategy for combining confidence scores.
    #[serde(default)]
    pub strategy: FusionStrategy,
    /// Per-method confidence scaling applied before fusion.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub calibration: CalibrationMap,
}
