//! [`DeduplicationStrategy`]: how to combine confidence scores when
//! fusing a group of co-referent entities.

use std::collections::HashMap;

use nvisy_core::entity::Entity;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Strategy for combining confidence scores from multiple detectors.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeduplicationStrategy {
    /// Take the maximum confidence across all detectors.
    #[default]
    MaxConfidence,
    /// Weighted average across the group: each entity's confidence
    /// contributes proportionally to its weight; the result is
    /// `Σ(wᵢ·pᵢ) / Σ(wᵢ)`.
    ///
    /// Per-entity weight = `max(weights[recognizer])` across the
    /// entity's recognition trail step sources. If an entity has no
    /// matching recognizer listed in `weights`, the per-entity weight
    /// floors at `1.0` (so an unweighted entity still counts, with
    /// neutral weight). `WeightedAverage { weights: {} }` therefore
    /// reduces to a plain unweighted average.
    WeightedAverage {
        /// Per-recognizer weight contributed to entities matched by
        /// that recognizer (keyed by the source name on the
        /// [`Recognition`]
        /// trail step). Recognizers missing from the map contribute
        /// the floor weight `1.0`.
        ///
        /// [`Recognition`]: nvisy_core::entity::TrailStepKind::Recognition
        weights: HashMap<String, f64>,
    },
    /// Noisy-OR: `P = 1 − ∏(1 − pᵢ)` for independent detectors.
    NoisyOr,
}

impl DeduplicationStrategy {
    /// Combine confidences across `group` per this strategy.
    pub(super) fn compute_confidence<M: nvisy_core::modality::Modality>(
        &self,
        group: &[Entity<M>],
    ) -> f64 {
        match self {
            Self::MaxConfidence => group
                .iter()
                .map(|e| e.confidence.get())
                .fold(f64::NEG_INFINITY, f64::max),

            Self::NoisyOr => {
                // P(at least one) = 1 − ∏(1 − pᵢ)
                1.0 - group
                    .iter()
                    .map(|e| 1.0 - e.confidence.get())
                    .product::<f64>()
            }

            Self::WeightedAverage { weights } => {
                let (wsum, total_w) =
                    group.iter().fold((0.0_f64, 0.0_f64), |(wsum, total_w), e| {
                        let w = e
                            .recognizers()
                            .filter_map(|name| weights.get(name).copied())
                            .fold(1.0_f64, f64::max);
                        (wsum + e.confidence.get() * w, total_w + w)
                    });
                if total_w > 0.0 { wsum / total_w } else { 0.0 }
            }
        }
    }
}
