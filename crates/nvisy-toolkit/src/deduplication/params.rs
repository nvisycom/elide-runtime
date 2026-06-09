//! [`LayerParams`]: the per-call knob bag that drives the
//! canonical deduplication recipe.
//!
//! Bundles every per-layer setting the four-step recipe needs
//! ([`CalibrationMap`], filtering thresholds + allowed kinds,
//! [`DeduplicationStrategy`], [`GroupingCriteria`],
//! [`ConflictResolution`]) into a single deserialisable shape
//! callers set once per request.
//! [`LayerPipeline::from_params`] reads it and assembles the
//! four-step pipeline.
//!
//! [`CalibrationMap`]: super::calibrate::CalibrationMap
//! [`DeduplicationStrategy`]: super::fuse::DeduplicationStrategy
//! [`GroupingCriteria`]: super::fuse::GroupingCriteria
//! [`ConflictResolution`]: super::resolve::ConflictResolution
//! [`LayerPipeline::from_params`]: super::pipeline::LayerPipeline::from_params

use nvisy_core::entity::EntityKind;
use nvisy_core::primitive::ConfidenceThreshold;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::calibrate::CalibrationMap;
use super::fuse::{DeduplicationStrategy, GroupingCriteria};
use super::resolve::ConflictResolution;

/// Configuration for the deduplication pipeline's four-step recipe.
///
/// Owns the sole confidence threshold in the pipeline: detection
/// layers and recognizers do not filter on confidence themselves —
/// per-method skew is folded in via [`calibration`], and the
/// resulting calibrated score is checked against
/// [`confidence_threshold`] here.
///
/// [`calibration`]: Self::calibration
/// [`confidence_threshold`]: Self::confidence_threshold
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct LayerParams {
    /// Per-recognizer confidence scaling applied first.
    #[serde(default, skip_serializing_if = "CalibrationMap::is_empty")]
    pub calibration: CalibrationMap,
    /// Drop entities whose `entity_kind` is outside this set. `None`
    /// keeps every kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_kinds: Option<Vec<EntityKind>>,
    /// Minimum calibrated confidence an entity must clear to survive
    /// the filter step. `None` keeps every confidence level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence_threshold: Option<ConfidenceThreshold>,
    /// How to match entity values and locations when grouping for
    /// fusion.
    #[serde(default)]
    pub grouping: GroupingCriteria,
    /// Strategy for combining confidence scores within a fused
    /// group.
    #[serde(default)]
    pub strategy: DeduplicationStrategy,
    /// How to resolve conflicts when different entity kinds overlap
    /// the same span.
    #[serde(default)]
    pub conflict_resolution: ConflictResolution,
}
