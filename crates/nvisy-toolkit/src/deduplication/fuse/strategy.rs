//! [`DeduplicationStrategy`]: how to combine confidence scores when
//! fusing a group of co-referent entities.

use std::collections::HashMap;

use nvisy_core::entity::Entity;
use nvisy_core::modality::Modality;
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
    /// Per-entity weight comes from the entity's originating
    /// recognizer — the source of its first `Recognition` trail
    /// step. (Each input entity at fuse time has its own recognition
    /// step(s) but hasn't been merged with siblings yet, so the
    /// first one is unambiguously what produced this entity.) If
    /// that recognizer isn't in `weights`, the per-entity weight
    /// defaults to `1.0`. `WeightedAverage { weights: {} }` reduces
    /// to a plain unweighted average.
    ///
    /// # Non-monotonicity
    ///
    /// Unlike [`MaxConfidence`] or [`NoisyOr`], the fused
    /// confidence can be *lower* than the highest input. A
    /// high-confidence hit from a low-weight detector gets pulled
    /// down by a lower-confidence hit from a high-weight detector.
    /// This is the price of expressing per-detector trust. If you
    /// want "more detectors agreeing should only increase
    /// confidence," use [`NoisyOr`] instead.
    ///
    /// [`MaxConfidence`]: Self::MaxConfidence
    /// [`NoisyOr`]: Self::NoisyOr
    WeightedAverage {
        /// Per-recognizer weight (keyed by the source name on the
        /// originating [`Recognition`] trail step). Recognizers
        /// missing from the map default to weight `1.0`.
        ///
        /// [`Recognition`]: nvisy_core::entity::TrailStepKind::Recognition
        weights: HashMap<String, f64>,
    },
    /// Weighted noisy-OR: `P = 1 − ∏(1 − pᵢ)^wᵢ`.
    ///
    /// Unweighted (`weights: {}`) reduces to standard noisy-OR
    /// `P = 1 − ∏(1 − pᵢ)`, which treats each detector as an
    /// independent witness — each contributes its own evidence and
    /// the joint probability that *all* are wrong is `∏(1 − pᵢ)`.
    ///
    /// # Correlation: use weights
    ///
    /// Independence is rarely literally true. Two regex patterns
    /// matching the same SSN format are ~100% correlated, not
    /// independent; standard noisy-OR will double-count and
    /// inflate. Set fractional weights on correlated detectors so
    /// each contributes only as much fresh evidence as it carries
    /// — two redundant patterns at `weights: { "pattern-a": 0.5,
    /// "pattern-b": 0.5 }` together count as one full witness.
    ///
    /// Per-entity weight uses the entity's originating recognizer
    /// (first `Recognition` trail step), matching [`WeightedAverage`].
    /// Missing recognizers contribute the standard unit weight
    /// `1.0` (vanilla noisy-OR contribution).
    ///
    /// [`WeightedAverage`]: Self::WeightedAverage
    NoisyOr {
        /// Per-recognizer evidence weight. Recognizers missing from
        /// the map contribute the unit weight `1.0`.
        #[serde(default)]
        weights: HashMap<String, f64>,
    },
}

impl DeduplicationStrategy {
    /// Combine confidences across `group` per this strategy.
    pub(super) fn compute_confidence<M: Modality>(&self, group: &[Entity<M>]) -> f64 {
        debug_assert!(
            !group.is_empty(),
            "fuse never calls compute_confidence with an empty group",
        );
        match self {
            Self::MaxConfidence => group
                .iter()
                .map(|e| e.confidence.get())
                .fold(f64::NEG_INFINITY, f64::max),

            Self::NoisyOr { weights } => {
                1.0 - group
                    .iter()
                    .map(|e| {
                        let w = entity_weight(e, weights);
                        (1.0 - e.confidence.get()).powf(w)
                    })
                    .product::<f64>()
            }

            Self::WeightedAverage { weights } => {
                let (wsum, total_w) = group.iter().fold((0.0_f64, 0.0_f64), |(s, t), e| {
                    let w = entity_weight(e, weights);
                    (s + e.confidence.get() * w, t + w)
                });
                wsum / total_w
            }
        }
    }
}

/// Look up the weight for an entity's originating recognizer (the
/// source of its first `Recognition` trail step). Missing-from-map
/// and missing-trail-step both fall through to the neutral weight
/// `1.0`.
fn entity_weight<M: Modality>(entity: &Entity<M>, weights: &HashMap<String, f64>) -> f64 {
    entity
        .recognizers()
        .next()
        .and_then(|name| weights.get(name))
        .copied()
        .unwrap_or(1.0)
}
