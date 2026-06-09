//! [`LayerPipeline`]: ordered stack of [`Layer`]s run head-to-tail.
//!
//! Build a pipeline either from canonical defaults
//! ([`LayerPipeline::from_params`]) or from individual layers
//! ([`LayerPipeline::new`] + [`LayerPipeline::with_layer`]). Run it
//! against an entity list with [`LayerPipeline::run`].

use std::marker::PhantomData;

use nvisy_core::entity::Entity;
use nvisy_core::extraction::TextAt;
use nvisy_core::modality::{Modality, Overlap};

use super::calibrate::CalibrateLayer;
use super::filter::{FilterLayer, FilterParams};
use super::fuse::FuseLayer;
use super::layer::{Layer, LayerContext};
use super::resolve::ResolveConflictsLayer;
use super::span_size::SpanSize;
use crate::deduplication::DeduplicationParams;

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
    /// Build the canonical four-layer dedup recipe: calibrate →
    /// filter → fuse → resolve. Each layer's config comes from
    /// `params`, except for the per-call [`FilterParams`] which
    /// carries the allowlist + threshold that aren't part of
    /// [`DeduplicationParams`] yet.
    pub fn from_params(params: &DeduplicationParams, filter: FilterParams) -> Self {
        Self::new()
            .with_layer(CalibrateLayer::new(params.calibration.clone()))
            .with_layer(FilterLayer::new(filter))
            .with_layer(FuseLayer::new(params.strategy.clone(), params.grouping))
            .with_layer(ResolveConflictsLayer::new(params.conflict_resolution))
    }
}

impl<M: Modality, R: TextAt<M> + ?Sized> Default for LayerPipeline<M, R> {
    fn default() -> Self {
        Self::new()
    }
}
