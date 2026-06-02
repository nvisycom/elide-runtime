//! [`Layer`]: one stage of an entity-processing pipeline.
//!
//! A layer takes a mutable entity collection and a read-only
//! [`LayerContext`], mutates the entities in place, and returns
//! whatever entities it dropped so the pipeline can roll up
//! drop-reason telemetry into a single log line. Layers that only
//! *add* to or *reshape* the collection return an empty `Vec`.
//!
//! The four built-in layers
//! ([`CalibrateLayer`],
//! [`FilterLayer`],
//! [`FuseLayer`],
//! [`ResolveConflictsLayer`]) cover the
//! canonical dedup recipe. Operators compose them — or their own
//! custom layers — through [`LayerPipeline`].
//!
//! # Generic over the resolver type
//!
//! Both `Layer<M, R>` and [`LayerContext<'_, M, R>`] are parameterised
//! by `R: ValueAt<M>` so the value-resolver call ([`fuse`]
//! is the only built-in caller) is monomorphised. Object safety still
//! holds — `LayerPipeline` stores `Box<dyn Layer<M, R>>` for a
//! specific `R` chosen at pipeline construction.
//!
//! [`CalibrateLayer`]: super::CalibrateLayer
//! [`FilterLayer`]: super::FilterLayer
//! [`FuseLayer`]: super::FuseLayer
//! [`ResolveConflictsLayer`]: super::ResolveConflictsLayer
//! [`LayerPipeline`]: super::LayerPipeline
//! [`fuse`]: super::FuseLayer

use async_trait::async_trait;
use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::Modality;
use uuid::Uuid;

use crate::core::ValueAt;

/// Read-only context every [`Layer::apply`] call receives.
///
/// Carries the value resolver (for layers that need to read source
/// text at a location — fuse, today) and an optional correlation id
/// (for tracing — see [`Context::correlation_id`]).
///
/// Built with [`Self::new`] then optionally extended through the
/// `with_*` setters:
///
/// ```ignore
/// let ctx = LayerContext::new(&resolver).with_correlation_id(run_id);
/// pipeline.run(entities, &ctx).await;
/// ```
///
/// [`Context::correlation_id`]: nvisy_core::Context
pub struct LayerContext<'a, M: Modality, R: ValueAt<M> + ?Sized> {
    /// Resolver for "what value sits at this location?". Layers that
    /// only inspect entity metadata can ignore this field.
    pub resolver: &'a R,
    /// Optional correlation id used to stitch tracing spans across
    /// the run. `None` means the layer (or pipeline) emits its spans
    /// without a correlation id, which is fine for ad-hoc / test
    /// usage.
    pub correlation_id: Option<Uuid>,
    /// Phantom binding `M` into the lifetime/type so the trait bound
    /// on `R` carries through without an unused-param error.
    _marker: std::marker::PhantomData<&'a M>,
}

impl<'a, M: Modality, R: ValueAt<M> + ?Sized> LayerContext<'a, M, R> {
    /// Construct a context with just a value resolver; correlation
    /// id defaults to `None`.
    pub fn new(resolver: &'a R) -> Self {
        Self {
            resolver,
            correlation_id: None,
            _marker: std::marker::PhantomData,
        }
    }

    /// Attach a correlation id (typically a run id).
    pub fn with_correlation_id(mut self, correlation_id: Uuid) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }
}

/// One stage of an entity-processing pipeline.
///
/// Each layer mutates `entities` in place; the returned `Vec` is the
/// list of entities the layer **dropped**. Layers that don't drop
/// anything (calibrate, fuse) return an empty vec; layers that filter
/// or resolve conflicts return the discarded entities so the pipeline
/// can roll up drop counts.
#[async_trait]
pub trait Layer<M: Modality, R: ValueAt<M> + ?Sized>: Send + Sync {
    /// Run this layer against `entities`. Returns the dropped vec.
    async fn apply(
        &self,
        entities: &mut Vec<Entity<M>>,
        ctx: &LayerContext<'_, M, R>,
    ) -> Vec<Entity<M>>;
}
