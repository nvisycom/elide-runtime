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
//! - [`Modality`] extends [`Modality`] with an associated `Data`
//!   type — the modality-specific payload (text bytes, image bytes +
//!   dims, …) recognizers actually scan.
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
//! [`Modality`]: crate::modality::Modality
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
/// Recognizers are expected to be stateless across calls. Any
/// per-document state a long-lived implementation needs is its
/// own responsibility to clear (e.g. an `Arc<Mutex<…>>` reset at
/// the top of `recognize`).
///
/// [`Modality`]: crate::modality::Modality
#[async_trait::async_trait]
pub trait EntityRecognizer<M: Modality>: Send + Sync {
    /// Detect entities in `input` and return them in modality-local
    /// coordinates. Downstream callers rebase text offsets into
    /// document coordinates when stitching results back into a
    /// multi-block document; image entities pass through unchanged.
    async fn recognize(&self, input: &RecognizerInput<M>) -> Result<RecognizerOutput<M>>;
}
