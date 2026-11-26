//! Deduplication pipeline: fuse → reconcile → threshold.
//!
//! Runs after recognition, on every modality. Assembles in
//! canonical order:
//!
//! 1. **Fuse**: merge overlapping findings that share a label into
//!    one entity spanning their union, at the highest confidence.
//! 2. **Reconcile cross-label**: pick a winner when overlapping
//!    entities carry different labels, using the
//!    highest-confidence tiebreaker.
//! 3. **Threshold**: drop entities below
//!    [`ConfidenceThreshold::BASELINE`].
//!
//! No calibration layer: no per-recognizer reweighting today.
//!
//! [`ConfidenceThreshold::BASELINE`]: elide::primitive::ConfidenceThreshold::BASELINE

use elide::detection::Analyzer;
use elide::detection::filter::ThresholdLayer;
use elide::detection::overlap::tiebreaker::HighestConfidence;
use elide::detection::overlap::{FuseLayer, ReconcileLayer, Structural};
use elide::modality::Modality;

/// Append the deduplication layers to `analyzer`.
pub(super) fn attach_dedup<M>(analyzer: Analyzer<M>) -> Analyzer<M>
where
    M: Modality,
{
    analyzer
        .with_layer(FuseLayer::max())
        .with_layer(ReconcileLayer::new(
            Structural::standard().with_tiebreaker(HighestConfidence),
        ))
        .with_layer(ThresholdLayer::default())
}
