//! Dedup-pipeline specs.
//!
//! Mirrors elide's `calibrate → reconcile → filter` layers. The
//! reconcile stage runs twice with different configurations
//! (once over same-label overlaps for merging and once over
//! cross-label overlaps for tiebreaking), each parameterised by
//! a serialisable strategy enum.

use std::collections::HashMap;

use elide_core::primitive::ConfidenceThreshold;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Dedup pipeline applied after recognition.
///
/// Layers run in the canonical order: calibrate → reconcile
/// (merging) → reconcile (tiebreaking) → filter.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeduplicationParams {
    /// Per-recognizer confidence weights.
    ///
    /// An empty map skips the calibrate layer. The engine
    /// converts this into elide's `CalibrationMap` at compile
    /// time.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub calibration: HashMap<String, f64>,
    /// How same-label overlapping findings are merged into one.
    ///
    /// Defaults to [`MergingStrategyParams::Max`].
    #[serde(default)]
    pub merging: MergingStrategyParams,
    /// How cross-label overlaps pick a winner.
    ///
    /// Defaults to [`TiebreakerParams::HighestConfidence`].
    #[serde(default)]
    pub tiebreaker: TiebreakerParams,
    /// Minimum confidence the filter layer admits.
    ///
    /// `None` falls back to [`ConfidenceThreshold::BASELINE`] at
    /// compile time. Wire form is an `f32` in `0.0..=1.0`;
    /// out-of-range values reject at deserialize, not silently
    /// clamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_confidence: Option<ConfidenceThreshold>,
}

/// How the merging reconciler combines same-label overlaps.
///
/// Picks one entity per overlapping cluster of findings that
/// share a label.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MergingStrategyParams {
    /// Take the maximum confidence across the cluster.
    #[default]
    Max,
    /// Probabilistic noisy-or combination.
    ///
    /// Treats recognizers as independent evidence sources.
    NoisyOr,
}

/// How the structural reconciler picks a winner across labels.
///
/// Runs after merging when overlapping entities carry different
/// labels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TiebreakerParams {
    /// Keep the highest-confidence entity.
    #[default]
    HighestConfidence,
    /// Keep the entity covering the longest span.
    LongestSpan,
}
