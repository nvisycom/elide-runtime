//! Per-modality detection dispatch (internal).
//!
//! [`DetectDispatch<M>`] is the modality-polymorphism plumbing
//! [`DetectionPhase<M>`] dispatches through. Each modality
//! (`text`/`tabular`/`image`/`audio`) has an impl on
//! [`DetectionEngine`] in [`super`] that resolves which recognizers
//! to run; the phase compiles against the trait bound and Rust
//! monomorphizes to the right one.
//!
//! Not exposed outside the engine: callers construct
//! [`DetectionPhase`] (the public surface) and never see this
//! trait.
//!
//! [`DetectionEngine`]: super::DetectionEngine
//! [`DetectionPhase<M>`]: super::DetectionPhase
//! [`DetectionPhase`]: super::DetectionPhase

use nvisy_core::Result;
use nvisy_ontology::modality::Modality;

use super::Detection;
use crate::pipeline::PhaseTarget;

/// Per-modality detection dispatch.
#[async_trait::async_trait]
pub trait DetectDispatch<M: Modality>: Send + Sync {
    async fn detect(&self, target: &mut PhaseTarget<'_, M>, cfg: &Detection) -> Result<()>;
}
