//! Entity deduplication: composable layers run through a
//! [`LayerPipeline`].
//!
//! # Layer submodules
//!
//! Each of the four canonical step kinds is its own public
//! submodule, named for what it does to the entity set:
//!
//! - [`calibrate`] — per-recognizer confidence scaling.
//! - [`filter`] — drop entities outside the allowed kinds or below
//!   a confidence floor.
//! - [`fuse`] — group + combine co-referent entities.
//! - [`resolve`] — break cross-kind span overlaps.
//!
//! # Plumbing (re-exported at the root)
//!
//! - [`Layer`] / [`LayerContext`] — the trait every step implements
//!   plus its per-call context.
//! - [`LayerPipeline`] — the orchestrator that runs a stack of
//!   layers in order.
//! - [`LayerParams`] — the per-call knob bag callers set.
//! - [`SpanSize`] — helper for span-length tiebreaks (used by
//!   `fuse` and `resolve` internally; exposed for custom layers).
//!
//! # Canonical recipe
//!
//! [`LayerPipeline::from_params`] assembles the canonical four-step
//! recipe from a [`LayerParams`]:
//!
//! 1. **Calibrate** raw confidence scores per-recognizer.
//! 2. **Filter** by allowed kinds + confidence floor.
//! 3. **Fuse** co-referent entities into one (group + combine).
//! 4. **Resolve conflicts** between different kinds on the same span.
//!
//! Operators can swap steps, drop steps, or insert their own custom
//! [`Layer`] impls by building the pipeline manually with
//! [`LayerPipeline::new`] + [`LayerPipeline::with_layer`].

pub mod calibrate;
pub mod filter;
pub mod fuse;
pub mod resolve;

mod layer;
mod params;
mod pipeline;
mod span_size;

pub use self::layer::{Layer, LayerContext};
pub use self::params::LayerParams;
pub use self::pipeline::LayerPipeline;
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
