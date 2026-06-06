//! [`EntityRecognizer<M>`]: the Presidio-style entity-detection trait.
//!
//! Every detector that emits [`Entity<M>`] for some modality `M`
//! implements this trait — pattern recognizers, NER bento clients,
//! LLM agents, OCR pipelines, plus any third-party recognizer a
//! consumer wires into their pipeline. Object-safe so heterogeneous
//! recognizers live behind `Arc<dyn EntityRecognizer<M>>` in
//! consumer-side registries.
//!
//! # Layering
//!
//! - [`crate::modality::Modality`] extends [`crate::modality::Modality`]
//!   with an associated `Data` type — the modality-specific payload
//!   (text bytes, image bytes + dims, …) recognizers actually scan.
//! - [`RecognizerInput<M>`] carries the payload plus the per-call
//!   concerns recognizers use ([`Artifacts`], language hints,
//!   candidate-language whitelist, uploader-supplied [`Hint<M>`]
//!   regions, document-level labels, correlation id). Location is
//!   intentionally absent — recognizers emit modality-local
//!   coordinates and don't read a per-call location; extractors use
//!   [`Span<M>`] instead.
//! - [`EntityRecognizer<M>`] takes `&RecognizerInput<M>` and emits a
//!   [`RecognizerOutput<M>`].
//!
//! [`Artifacts`]: crate::extraction::Artifacts
//! [`Entity<M>`]: crate::entity::Entity
//! [`Span<M>`]: crate::extraction::Span

mod hint;
mod input;
mod label_map;
mod output;

pub use self::hint::Hint;
pub use self::input::RecognizerInput;
pub use self::label_map::LabelMap;
pub use self::output::RecognizerOutput;
use crate::Result;
use crate::modality::Modality;

/// Recognizer for a single [`Modality`] `M`.
///
/// Implementors emit a [`RecognizerOutput<M>`] for one document or
/// one scan unit, reading whatever per-call configuration they need
/// from [`RecognizerInput<M>`]. Each consumer composes their own
/// list of recognizers; the trait does not assume a central
/// registry.
///
/// Recognizers are stateless from the caller's perspective — the
/// default [`reset`] is a no-op. Long-lived implementations (LLM
/// agents with cumulative usage trackers, OCR backends with batch
/// caches) override `reset` to drop per-document state between
/// runs.
///
/// [`Modality`]: crate::modality::Modality
/// [`reset`]: Self::reset
#[async_trait::async_trait]
pub trait EntityRecognizer<M: Modality>: Send + Sync {
    /// Detect entities in `input` and return them in modality-local
    /// coordinates. Downstream callers rebase text offsets into
    /// document coordinates when stitching results back into a
    /// multi-block document; image entities pass through unchanged.
    async fn recognize(&self, input: &RecognizerInput<M>) -> Result<RecognizerOutput<M>>;

    /// Drop per-document state. Default no-op for stateless
    /// recognizers; long-lived ones (usage trackers, batch caches)
    /// override.
    async fn reset(&self) {}
}
