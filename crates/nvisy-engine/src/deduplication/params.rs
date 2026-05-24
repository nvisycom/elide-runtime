//! [`DeduplicationParams`]: deduplication node configuration bundle.
//!
//! Aggregates the per-layer configuration types
//! ([`CalibrationMap`], [`DeduplicationStrategy`],
//! [`GroupingCriteria`], [`ConflictResolution`]) into the shape the
//! workflow ingests as a single JSON section.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::calibrate::CalibrationMap;
use super::fuse::{DeduplicationStrategy, GroupingCriteria};
use super::resolve::ConflictResolution;

/// Configuration for the deduplication phase.
///
/// Merges and scores entity candidates from multiple detection
/// sources into a deduplicated, confidence-scored entity list.
///
/// The minimum confidence threshold isn't on this struct — it comes
/// from the operator's per-call [`Detection`] config via
/// [`FilterParams`], so the operator controls filtering uniformly.
///
/// [`Detection`]: crate::detection::Detection
/// [`FilterParams`]: super::FilterParams
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DeduplicationParams {
    /// How to match entity values and locations when grouping.
    #[serde(default)]
    pub grouping: GroupingCriteria,
    /// Strategy for combining confidence scores.
    #[serde(default)]
    pub strategy: DeduplicationStrategy,
    /// Per-method confidence scaling applied before deduplication.
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub calibration: CalibrationMap,
    /// How to resolve conflicts when different entity kinds overlap
    /// the same text span.
    #[serde(default)]
    pub conflict_resolution: ConflictResolution,
}
