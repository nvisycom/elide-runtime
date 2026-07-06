//! Deduplication pipeline: calibrate → reconcile → filter.
//!
//! Runs after recognition, on every modality. Wire spec is
//! [`nvisy_schema::plan::DeduplicationParams`]. Layers assemble
//! in canonical order:
//!
//! 1. **Calibrate** — reweight recognizer scores against the
//!    per-request calibration map (skipped when the map is
//!    empty).
//! 2. **Reconcile same-label** — merge overlapping findings that
//!    share a label into one entity, using either max-confidence
//!    or noisy-or.
//! 3. **Reconcile cross-label** — pick a winner when overlapping
//!    entities carry different labels, using either
//!    highest-confidence or longest-span tiebreaker.
//! 4. **Filter** — drop entities below the min-confidence
//!    threshold (falls back to
//!    [`ConfidenceThreshold::BASELINE`] when unset).
//!
//! [`ConfidenceThreshold::BASELINE`]: elide_core::primitive::ConfidenceThreshold::BASELINE

use elide::detection::Analyzer;
use elide::detection::calibrate::CalibrateLayer;
use elide::detection::filter::FilterLayer;
use elide::detection::reconcile::scoring::{MaxConfidence, NoisyOrConfidence};
use elide::detection::reconcile::tiebreaker::{HighestConfidence, LongestSpan};
use elide::detection::reconcile::{Merging, ReconcileLayer, Structural};
use elide_core::modality::Modality;
use elide_core::primitive::ConfidenceThreshold;
use nvisy_schema::plan::{DeduplicationParams, MergingStrategyParams, TiebreakerParams};

/// Append the deduplication layers to `analyzer`.
///
/// See the module doc for the layer order. Calibrate is skipped
/// when the calibration map is empty.
pub(in crate::analyzer) fn attach<M>(
    mut analyzer: Analyzer<M>,
    spec: &DeduplicationParams,
) -> Analyzer<M>
where
    M: Modality,
{
    if !spec.calibration.is_empty() {
        // Wire type is `HashMap<String, f64>`; elide's
        // `CalibrationMap` is `FromIterator<(K, V)>` where K:
        // Into<String> and V: Into<f64>.
        analyzer = analyzer.with_layer(CalibrateLayer::new(
            spec.calibration
                .iter()
                .map(|(k, &v)| (k.clone(), v))
                .collect(),
        ));
    }

    analyzer = match spec.merging {
        MergingStrategyParams::Max => {
            analyzer.with_layer(ReconcileLayer::same_label(Merging::new(MaxConfidence)))
        }
        MergingStrategyParams::NoisyOr => {
            analyzer.with_layer(ReconcileLayer::same_label(Merging::new(NoisyOrConfidence)))
        }
    };

    analyzer = match spec.tiebreaker {
        TiebreakerParams::HighestConfidence => analyzer.with_layer(ReconcileLayer::cross_label(
            Structural::standard().with_tiebreaker(HighestConfidence),
        )),
        TiebreakerParams::LongestSpan => analyzer.with_layer(ReconcileLayer::cross_label(
            Structural::standard().with_tiebreaker(LongestSpan),
        )),
    };

    let threshold = spec.min_confidence.unwrap_or(ConfidenceThreshold::BASELINE);
    analyzer.with_layer(FilterLayer::new().with_threshold(threshold))
}
