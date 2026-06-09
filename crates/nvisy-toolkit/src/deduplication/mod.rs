//! Entity deduplication: composable layers run through a
//! [`LayerPipeline`].
//!
//! Public surface is the [`Layer`] trait, the four built-in layer
//! types ([`CalibrateLayer`], [`FilterLayer`], [`FuseLayer`],
//! [`ResolveConflictsLayer`]), the [`LayerPipeline`] orchestrator,
//! and the per-layer config types ([`CalibrationMap`], [`FilterParams`],
//! [`DeduplicationStrategy`], [`GroupingCriteria`],
//! [`ConflictResolution`], [`SpanSize`]).
//!
//! The phase orchestrator that drives this per `DocumentTree` node
//! lives in `nvisy_engine::phases::deduplication::DeduplicationPhase`.
//! It calls [`LayerPipeline::from_params`] to assemble the canonical
//! four-step recipe, then runs the pipeline against each node's
//! entities.
//!
//! # Canonical recipe
//!
//! 1. **Calibrate** raw confidence scores per-recognizer.
//! 2. **Filter** by allowed kinds + confidence floor.
//! 3. **Fuse** co-referent entities into one (group + combine).
//! 4. **Resolve conflicts** between different kinds on the same span.
//!
//! Operators can swap steps, drop steps, or insert their own custom
//! [`Layer`] impls by building the pipeline manually with
//! [`LayerPipeline::new`] + [`LayerPipeline::with_layer`].
//!
//! `DocumentTree` and `DeduplicationPhase` live in `nvisy-engine`.

mod calibrate;
pub mod config;
mod filter;
mod fuse;
mod layer;
mod pipeline;
mod resolve;
mod span_size;

pub use self::calibrate::{CalibrateLayer, CalibrationMap};
pub use self::config::DeduplicationParams;
pub use self::filter::{FilterLayer, FilterParams};
pub use self::fuse::{DeduplicationStrategy, FuseLayer, GroupingCriteria};
pub use self::layer::{Layer, LayerContext};
pub use self::pipeline::LayerPipeline;
pub use self::resolve::{ConflictResolution, ResolveConflictsLayer};
pub use self::span_size::SpanSize;

#[cfg(test)]
pub(crate) fn test_resolver<M: nvisy_core::modality::Modality>()
-> Box<dyn nvisy_core::extraction::TextAt<M>> {
    use async_trait::async_trait;
    use nvisy_core::extraction::TextAt;
    use nvisy_core::modality::Modality;

    struct Noop<M>(std::marker::PhantomData<M>);

    #[async_trait]
    impl<M: Modality> TextAt<M> for Noop<M> {
        async fn text_at(&self, _location: &M::Location) -> Option<String> {
            None
        }
    }

    Box::new(Noop::<M>(std::marker::PhantomData))
}
