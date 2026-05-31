//! Per-modality extraction dispatch (internal).
//!
//! [`ExtractDispatch<M>`] is the modality-polymorphism plumbing
//! [`ExtractionPhase<M>`] dispatches through. Each modality
//! (`text`/`tabular`/`image`/`audio`) implements it on
//! [`ExtractionEngine`]; the phase resolves the right impl at compile
//! time via the `ExtractionEngine: ExtractDispatch<M>` bound.
//!
//! Not exposed outside the engine: callers construct
//! [`ExtractionPhase`] (which is the public surface) and never see
//! this trait directly.
//!
//! [`ExtractionEngine`]: super::ExtractionEngine
//! [`ExtractionPhase<M>`]: super::ExtractionPhase
//! [`ExtractionPhase`]: super::ExtractionPhase

use nvisy_core::Result;
use nvisy_ontology::modality::Modality;

use super::ExtractionEngine;
use crate::pipeline::PhaseTarget;

/// Per-modality extraction dispatch.
///
/// One impl per `M` on [`ExtractionEngine`]; the per-modality submodule
/// (`text`/`tabular`/`image`/`audio`) provides the body. Plan type is
/// per-`M` via the [`Self::Plan`] associated type.
#[async_trait::async_trait]
pub trait ExtractDispatch<M: Modality>: Send + Sync {
    /// Per-modality plan slice.
    type Plan: Default + Send + Sync;

    async fn extract(&self, target: &mut PhaseTarget<'_, M>, plan: &Self::Plan) -> Result<()>;
}

/// Helper that picks the per-modality plan slice out of
/// [`Extraction`]. One impl per modality, co-located with each
/// modality's [`ExtractDispatch<M>`] impl.
///
/// [`Extraction`]: super::Extraction
pub trait PlanSlice<M: Modality>
where
    ExtractionEngine: ExtractDispatch<M>,
{
    fn slice(&self) -> &<ExtractionEngine as ExtractDispatch<M>>::Plan;
}
