//! Deduplication pipeline: reconcile → filter.
//!
//! Runs after recognition, on every modality. Assembles in
//! canonical order:
//!
//! 1. **Reconcile same-label**: merge overlapping findings that
//!    share a label into one entity using max-confidence.
//! 2. **Reconcile cross-label**: pick a winner when overlapping
//!    entities carry different labels, using the
//!    highest-confidence tiebreaker.
//! 3. **Filter**: drop entities below
//!    [`ConfidenceThreshold::BASELINE`].
//!
//! No calibration layer: no per-recognizer reweighting today.
//!
//! [`ConfidenceThreshold::BASELINE`]: elide_core::primitive::ConfidenceThreshold::BASELINE

use elide::detection::Analyzer;
use elide::detection::filter::FilterLayer;
use elide::detection::reconcile::scoring::MaxConfidence;
use elide::detection::reconcile::tiebreaker::HighestConfidence;
use elide::detection::reconcile::{Merging, ReconcileLayer, Structural};
use elide_core::modality::Modality;
use elide_core::primitive::ConfidenceThreshold;

/// Append the deduplication layers to `analyzer`.
pub(super) fn attach_dedup<M>(analyzer: Analyzer<M>) -> Analyzer<M>
where
    M: Modality,
{
    analyzer
        .with_layer(ReconcileLayer::same_label(Merging::new(MaxConfidence)))
        .with_layer(ReconcileLayer::cross_label(
            Structural::standard().with_tiebreaker(HighestConfidence),
        ))
        .with_layer(FilterLayer::new().with_threshold(ConfidenceThreshold::BASELINE))
}
