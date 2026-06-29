//! Dedup-pipeline specs.
//!
//! Mirrors elide's `calibrate → reconcile → filter` layers. The
//! reconcile stage runs twice with different configurations —
//! once over same-label overlaps (merging) and once over
//! cross-label overlaps (tiebreaking) — each parameterised by a
//! serialisable strategy enum.

use elide::detection::calibrate::CalibrationMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Dedup pipeline applied after recognition. Layers run in the
/// canonical order: calibrate → reconcile (merging) → reconcile
/// (tiebreaking) → filter.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeduplicationParams {
    /// Per-recognizer confidence weights. An empty map skips the
    /// calibrate layer.
    #[serde(default, skip_serializing_if = "CalibrationMap::is_empty")]
    pub calibration: CalibrationMap,
    /// How same-label overlapping findings are merged into one.
    /// Defaults to [`MergingStrategyParams::Max`].
    #[serde(default)]
    pub merging: MergingStrategyParams,
    /// How cross-label overlaps pick a winner. Defaults to
    /// [`TiebreakerParams::HighestConfidence`].
    #[serde(default)]
    pub tiebreaker: TiebreakerParams,
    /// Minimum confidence the filter layer admits. `None` falls
    /// back to elide's `ConfidenceThreshold::BASELINE` at compile
    /// time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_confidence: Option<f32>,
}

/// Strategy the merging reconciler uses to combine same-label
/// overlapping findings into one entity.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MergingStrategyParams {
    /// Take the maximum confidence across the cluster.
    #[default]
    Max,
    /// Probabilistic noisy-or combination — treats recognizers as
    /// independent evidence sources.
    NoisyOr,
}

/// Tiebreaker the structural reconciler uses to pick a winner
/// when overlapping entities carry different labels.
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
