//! [`LayerPipeline`]: ordered stack of [`Layer`]s run head-to-tail.
//!
//! Build a pipeline either from canonical defaults
//! ([`LayerPipeline::from_params`]) or from individual layers
//! ([`LayerPipeline::new`] + [`LayerPipeline::with_layer`]). Run it
//! against an entity list with [`LayerPipeline::run`].

use std::marker::PhantomData;

use nvisy_core::Error;
use nvisy_core::entity::Entity;
use nvisy_core::extraction::TextAt;
use nvisy_core::modality::{Modality, Overlap};

use super::calibrate::CalibrateLayer;
use super::filter::FilterLayer;
use super::fuse::FuseLayer;
use super::layer::{Layer, LayerContext};
use super::params::LayerParams;
use super::resolve::ResolveConflictsLayer;
use super::span_size::SpanSize;
use super::suppress::SuppressionLayer;

const TARGET: &str = "nvisy_toolkit::deduplication";

/// Ordered stack of layers, run head-to-tail against an entity list.
///
/// Construction is open: callers can compose any sequence of
/// [`Layer<M, R>`] impls (built-in or custom). The canonical
/// four-layer dedup recipe is available through [`Self::from_params`].
///
/// `R` is the resolver type passed in through the [`LayerContext`] at
/// `run` time. It's a type parameter on the pipeline (rather than a
/// `dyn TextAt<M>`) so each layer's `text_at` call is
/// monomorphised. Layers that don't touch the resolver compile
/// against any `R: TextAt<M>` uniformly.
pub struct LayerPipeline<M: Modality, R: TextAt<M> + ?Sized> {
    layers: Vec<Box<dyn Layer<M, R>>>,
    _marker: PhantomData<fn(&M, &R)>,
}

impl<M: Modality, R: TextAt<M> + ?Sized> LayerPipeline<M, R> {
    /// Empty pipeline. Use [`Self::with_layer`] to append layers.
    pub fn new() -> Self {
        Self {
            layers: Vec::new(),
            _marker: PhantomData,
        }
    }

    /// Append a layer.
    pub fn with_layer<L: Layer<M, R> + 'static>(mut self, layer: L) -> Self {
        self.layers.push(Box::new(layer));
        self
    }

    /// Run every layer in registration order against `entities`.
    ///
    /// The pipeline aggregates each layer's dropped count into a
    /// single info log line per run. Layers themselves emit per-layer
    /// debug/trace spans.
    pub async fn run(
        &self,
        mut entities: Vec<Entity<M>>,
        ctx: &LayerContext<'_, M, R>,
    ) -> Vec<Entity<M>> {
        if entities.is_empty() {
            return entities;
        }
        let before = entities.len();

        let mut dropped_total = 0usize;
        for layer in &self.layers {
            let dropped = layer.apply(&mut entities, ctx).await;
            dropped_total += dropped.len();
        }

        tracing::info!(
            target: TARGET,
            before,
            after = entities.len(),
            reduced = before.saturating_sub(entities.len()),
            dropped = dropped_total,
            "layer pipeline complete",
        );

        entities
    }
}

impl<M, R> LayerPipeline<M, R>
where
    M: Modality,
    M::Location: Overlap + SpanSize,
    R: TextAt<M> + ?Sized,
{
    /// Build the canonical five-layer recipe: calibrate → filter →
    /// fuse → suppress → resolve. Every layer's config is read
    /// from `params`.
    ///
    /// # Errors
    ///
    /// Returns a validation error when any
    /// `params.allow_values_regex` entry fails to compile.
    pub fn from_params(params: &LayerParams) -> Result<Self, Error> {
        let filter = FilterLayer::new()
            .with_allowed_labels(params.allowed_labels.clone())
            .with_confidence_threshold(params.confidence_threshold);
        let suppress = SuppressionLayer::from_params(&params.suppression)?;
        Ok(Self::new()
            .with_layer(CalibrateLayer::new(params.calibration.clone()))
            .with_layer(filter)
            .with_layer(FuseLayer::new(params.strategy.clone(), params.grouping))
            .with_layer(suppress)
            .with_layer(ResolveConflictsLayer::new(params.conflict_resolution)))
    }
}

impl<M: Modality, R: TextAt<M> + ?Sized> Default for LayerPipeline<M, R> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use nvisy_core::entity::{Entity, builtins};
    use nvisy_core::modality::{Text, TextLocation};

    use super::*;
    use crate::deduplication::suppress::SuppressionParams;

    struct TextSliceResolver(Arc<String>);

    #[async_trait]
    impl TextAt<Text> for TextSliceResolver {
        async fn text_at(&self, location: &TextLocation) -> Option<String> {
            self.0.get(location.start..location.end).map(String::from)
        }
    }

    fn email(start: usize, end: usize) -> Entity<Text> {
        Entity::test_builder(start, end)
            .with_label(builtins::EMAIL_ADDRESS.label_ref())
            .test_build()
    }

    fn url(start: usize, end: usize) -> Entity<Text> {
        Entity::test_builder(start, end)
            .with_label(builtins::URL.label_ref())
            .test_build()
    }

    /// Pipeline-order contract: fuse collapses same-kind duplicates
    /// before suppress sees them; suppress drops allowlisted
    /// entities before resolve adjudicates cross-kind conflicts.
    ///
    /// Inputs: two PERSON_NAME hits at the same span (duplicates
    /// of an allowlisted email-like value) plus one URL hit at an
    /// overlapping span. After the pipeline, only the URL should
    /// survive.
    ///
    /// Without `fuse → suppress` ordering, the duplicate would
    /// survive (suppress only drops one of the two). Without
    /// `suppress → resolve` ordering, the resolve step would
    /// pick a winner between EMAIL and URL — possibly the EMAIL —
    /// before the allow-list could remove it.
    #[tokio::test]
    async fn fuse_then_suppress_then_resolve() {
        let source = "noreply@foo.com /docs";
        let resolver = TextSliceResolver(Arc::new(source.to_owned()));

        let params = LayerParams {
            suppression: SuppressionParams::new()
                .with_allow_values(vec!["noreply@foo.com".to_owned()]),
            ..Default::default()
        };

        let pipeline: LayerPipeline<Text, _> =
            LayerPipeline::from_params(&params).expect("pipeline builds");
        let ctx = LayerContext::new(&resolver);

        // Two EMAIL hits at [0, 15) — same kind, same span, fuse
        // collapses to one. The collapsed entity matches the
        // allow-list; suppress drops it. A URL at [0, 21) remains
        // for resolve to leave untouched.
        let entities = vec![email(0, 15), email(0, 15), url(0, 21)];

        let survivors = pipeline.run(entities, &ctx).await;
        assert_eq!(survivors.len(), 1, "only the URL should survive");
        assert_eq!(survivors[0].label, builtins::URL.label_ref());
    }
}
