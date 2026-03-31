//! Post-detection refinement method classification.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

/// Post-detection refinement applied to an entity before final output.
///
/// Refinement methods do not discover new entities: they adjust
/// confidence, merge duplicates, or verify existing detections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Display, EnumString, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[non_exhaustive]
pub enum RefinementMethod {
    /// Cross-detector deduplication.
    Deduplication,
    /// Ensemble fusion: combines confidence scores from multiple detectors.
    EnsembleFusion,
    /// Model-based verification: a secondary model reviews detections.
    ModelVerification,
    /// Policy evaluation: applies business rules or thresholds.
    PolicyEvaluation,
    /// Human review.
    HumanReview,
    /// Confidence calibration.
    ConfidenceCalibration,
    /// Contextual promotion/demotion.
    ContextualAdjustment,
}
