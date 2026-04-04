//! Deduplication node configuration and strategy.

use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::entity::RecognitionMethodKind;

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
    /// across non-overlapping regions (e.g. cross-chunk deduplication).
    Widening,
}

impl GroupingCriteria {
    /// Whether two values match under this criteria.
    pub fn values_match(self, a: &str, b: &str) -> bool {
        match self {
            Self::Strict => a == b,
            Self::Normalized => a.trim().eq_ignore_ascii_case(b.trim()),
            Self::Narrowing | Self::Widening => {
                let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
                long.contains(short)
            }
        }
    }

    /// Whether this criteria requires overlapping locations for grouping.
    pub fn requires_location_overlap(self) -> bool {
        !matches!(self, Self::Widening)
    }

    /// Whether this criteria uses substring containment for value matching.
    pub fn is_substring(self) -> bool {
        matches!(self, Self::Narrowing | Self::Widening)
    }

    /// Normalise a value for HashMap bucketing under this criteria.
    pub fn bucket_value(self, value: &str) -> String {
        match self {
            Self::Normalized => value.trim().to_lowercase(),
            _ => value.to_owned(),
        }
    }
}

/// Strategy for combining confidence scores from multiple detectors.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum DeduplicationStrategy {
    /// Take the maximum confidence across all detectors.
    #[default]
    MaxConfidence,
    /// Weighted average by recognition method.
    WeightedAverage {
        /// Per-method weight (missing methods default to 1.0).
        weights: HashMap<RecognitionMethodKind, f64>,
    },
    /// Noisy-OR: `P = 1 − ∏(1 − pᵢ)` for independent detectors.
    NoisyOr,
}

/// Per-method confidence multiplier applied before deduplication.
///
/// Maps a [`RecognitionMethodKind`] to a scaling factor. Methods not
/// present in the map are left unchanged (implicit multiplier of 1.0).
pub type CalibrationMap = HashMap<RecognitionMethodKind, f64>;

/// Configuration for the deduplication graph node.
///
/// Merges and scores entity candidates from multiple detection
/// sources into a deduplicated, confidence-scored entity list.
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Deduplication {
    /// How to match entity values and locations when grouping.
    #[serde(default)]
    pub grouping: GroupingCriteria,
    /// Strategy for combining confidence scores.
    #[serde(default)]
    pub strategy: DeduplicationStrategy,
    /// Per-method confidence scaling applied before deduplication.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub calibration: CalibrationMap,
}
