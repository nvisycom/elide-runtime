//! Extraction phase: per-modality extractors + shared registry.
//!
//! Public surface is [`ExtractionPhase`] (plus the engine + config
//! re-exported from the `engine` submodule). Per-modality dispatch
//! ([`ExtractDispatch<M>`] + [`WorkflowSlice<M>`], in the private
//! `dispatch` module) is internal plumbing that the phase routes
//! through.
//!
//! Per-modality behaviour:
//!
//! - `text` / `tabular` — codec-native; no backend call.
//! - `image` — OCR (when `image` feature is on).
//! - `audio` — STT (when `audio` feature is on).
//!
//! [`ExtractDispatch<M>`]: dispatch::ExtractDispatch
//! [`WorkflowSlice<M>`]: dispatch::WorkflowSlice

mod audio;
mod dispatch;
mod engine;
mod image;
mod tabular;
mod text;
mod workflow;

use std::marker::PhantomData;

use async_trait::async_trait;
use nvisy_core::Result;
use nvisy_ontology::modality::Modality;

#[cfg(feature = "audio")]
pub use self::audio::{SttExtractor, SttExtractorConfig};
pub use self::dispatch::{ExtractDispatch, WorkflowSlice};
pub use self::engine::{ExtractionConfig, ExtractionEngine};
#[cfg(feature = "image")]
pub use self::image::{OcrExtractor, OcrExtractorConfig};
pub use self::workflow::{
    AudialWorkflow, Extraction, ImageWorkflow, TabularWorkflow, TextWorkflow,
};
use crate::envelope::DocumentEnvelope;
use crate::pipeline::{ModalityKind, Phase, PhaseContext, PhaseInfo};

impl Extraction {
    /// Borrow the per-modality workflow slice keyed by `M`.
    ///
    /// Used by [`ExtractionPhase`] to fish the matching workflow
    /// field out of this aggregate without each call site naming it
    /// explicitly.
    pub(crate) fn workflow_for<M>(&self) -> &<ExtractionEngine as ExtractDispatch<M>>::Workflow
    where
        M: Modality,
        ExtractionEngine: ExtractDispatch<M>,
        Self: WorkflowSlice<M>,
    {
        <Self as WorkflowSlice<M>>::slice(self)
    }
}

/// Extraction phase: walks the codec handle, populates
/// `envelope.document.blocks`.
///
/// Stateless beyond the modality marker — the shared
/// [`ExtractionEngine`] is read from `ctx.run` each call, and the
/// per-call workflow comes from `ctx.plan.extraction` via the
/// per-modality [`WorkflowSlice`] impl on [`Extraction`].
pub struct ExtractionPhase<M: Modality> {
    _marker: PhantomData<fn() -> M>,
}

impl<M: Modality> ExtractionPhase<M> {
    /// Build the phase. Stateless beyond the modality marker.
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<M: Modality> Default for ExtractionPhase<M> {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl<M> Phase<M> for ExtractionPhase<M>
where
    M: Modality,
    ExtractionEngine: ExtractDispatch<M>,
    Extraction: WorkflowSlice<M>,
{
    fn inspect(&self) -> PhaseInfo {
        PhaseInfo {
            name: "extraction",
            modality: ModalityKind::of::<M>(),
            mutating: true,
        }
    }

    async fn run(
        &self,
        ctx: &PhaseContext<'_, M>,
        envelope: &mut DocumentEnvelope<M>,
    ) -> Result<()> {
        let workflow = ctx.plan.extraction.workflow_for::<M>();
        ExtractDispatch::<M>::extract(ctx.run.extraction_engine.as_ref(), envelope, workflow).await
    }
}
