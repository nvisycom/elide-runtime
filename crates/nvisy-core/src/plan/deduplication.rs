//! Dedup-pipeline specs.
//!
//! Mirrors elide's [`Calibrate → Fuse → Resolve → Filter`] layers.
//! Each component is parameterised by a serialisable strategy enum;
//! engine maps each spec variant to the matching elide strategy at
//! compile time.

use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Dedup pipeline applied after recognition. Layers run in the
/// canonical order: calibrate → fuse → resolve → filter.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeduplicationSpec {
    /// Per-recognizer confidence weights. An empty map skips the
    /// calibrate layer.
    #[serde(default, skip_serializing_if = "CalibrationMap::is_empty")]
    pub calibration: CalibrationMap,
    /// How overlapping entities are fused into one. Defaults to
    /// [`FusionStrategySpec::MaxConfidence`].
    #[serde(default)]
    pub fusion: FusionStrategySpec,
    /// How an entity's identity is resolved when several remain
    /// after fusion. Defaults to
    /// [`ResolutionStrategySpec::HighestConfidence`].
    #[serde(default)]
    pub resolution: ResolutionStrategySpec,
    /// Minimum confidence the filter layer admits. `None` falls
    /// back to elide's `ConfidenceThreshold::BASELINE` at compile
    /// time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_confidence: Option<f32>,
}

/// Per-recognizer calibration weights (recognizer-name → multiplier).
///
/// Wire shape: `{ "pattern": 1.0, "ner": 0.85 }`. Engine builds the
/// elide [`CalibrationMap`] from this; absent recognizers default to
/// `1.0` (identity).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct CalibrationMap(pub HashMap<String, f64>);

impl CalibrationMap {
    /// `true` when no weights are set; engine skips the calibrate
    /// layer in that case.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Strategy the fuse layer uses to combine overlapping entities.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FusionStrategySpec {
    /// Keep the entity with the highest confidence.
    #[default]
    MaxConfidence,
    /// Arithmetic mean of overlapping confidences.
    Mean,
    /// Probabilistic noisy-or combination — treats recognizers as
    /// independent evidence sources.
    NoisyOr,
}

/// Strategy the resolve layer uses to break ties when several
/// entities cover the same span.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionStrategySpec {
    /// Keep the highest-confidence entity.
    #[default]
    HighestConfidence,
    /// Keep the entity covering the longest span.
    LongestSpan,
}
