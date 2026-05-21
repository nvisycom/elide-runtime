//! Reusable multi-stage agent pipelines.
//!
//! A pipeline composes one or more agents (and any state shared
//! across calls) into a single end-to-end flow. Today the only
//! built-in pipeline is [`NerPipeline`] (detect with [`NerAgent`]
//! → verify with [`NerVerifier`] → merge surviving candidates
//! into coreference state). Future pipelines (CV-verifier flow,
//! audio transcription + LLM cleanup) live as sibling modules.
//!
//! All pipelines implement the [`Pipeline`] trait, which currently
//! exposes a single document-boundary hook: [`reset`]. The trait
//! will grow uniform run/name methods as additional concrete
//! pipelines arrive.
//!
//! [`NerAgent`]: crate::agent::NerAgent
//! [`NerVerifier`]: crate::agent::NerVerifier
//! [`reset`]: Pipeline::reset

mod ner;

use async_trait::async_trait;

pub use self::ner::{NerPipeline, NerPipelineBuilder, NerPipelineBuilderError};

/// Cross-cutting hook every pipeline must implement: clear state
/// at document boundaries.
///
/// LLM-backed pipelines accumulate coreference across calls within
/// a document so the model can reuse stable entity ids. Between
/// documents that state must be cleared so identities don't bleed.
/// Stateless pipelines implement [`reset`] as a no-op.
///
/// [`reset`]: Self::reset
#[async_trait]
pub trait Pipeline: Send + Sync {
    /// Clear per-document state. Called by the orchestrator at
    /// document boundaries.
    async fn reset(&self);
}
