//! Deduplication plan node.
//!
//! [`DeduplicationParams`] runs at **phase 2**, after detection.
//! Aggregates the per-layer configuration types
//! ([`CalibrationMap`], [`DeduplicationStrategy`],
//! [`GroupingCriteria`], [`ConflictResolution`]) into the shape the
//! plan ingests as a single JSON section.
//!
//! The per-layer types themselves live in
//! [`crate::deduplication`] — they're the dedup algorithm's domain
//! types and aren't pure config. Only the plan-level bundle lives
//! here.
//!
//! [`CalibrationMap`]: crate::deduplication::CalibrationMap
//! [`DeduplicationStrategy`]: crate::deduplication::DeduplicationStrategy
//! [`GroupingCriteria`]: crate::deduplication::GroupingCriteria
//! [`ConflictResolution`]: crate::deduplication::ConflictResolution

use nvisy_ontology::primitive::ConfidenceThreshold;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::deduplication::{
    CalibrationMap, ConflictResolution, DeduplicationStrategy, GroupingCriteria,
};

/// Configuration for the deduplication phase.
///
/// Merges and scores entity candidates from multiple detection
/// sources into a deduplicated, confidence-scored entity list.
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
pub struct DeduplicationParams {
    /// How to match entity values and locations when grouping.
    #[serde(default)]
    pub grouping: GroupingCriteria,
    /// Strategy for combining confidence scores.
    #[serde(default)]
    pub strategy: DeduplicationStrategy,
    /// Per-method confidence scaling applied before filtering.
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub calibration: CalibrationMap,
    /// Minimum (calibrated) confidence an entity must clear to
    /// survive deduplication. `None` keeps every candidate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence_threshold: Option<ConfidenceThreshold>,
    /// How to resolve conflicts when different entity kinds overlap
    /// the same text span.
    #[serde(default)]
    pub conflict_resolution: ConflictResolution,
}
