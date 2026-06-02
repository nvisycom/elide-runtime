//! [`GlinerBackend`]: transport trait for zero-shot NER backends.
//!
//! Used by [`GlinerRecognizer`](crate::recognition::GlinerRecognizer).
//! The recognizer owns the post-processing — label normalization,
//! score demotion, aggregation — and dispatches the actual model
//! call through this trait.
//!
//! The wire contract is intentionally narrow: text + requested
//! `EntityKind`s in, raw model output (label + score + offsets)
//! out. Anything more (chunking, batching, retries) belongs in the
//! backend implementation.

use async_trait::async_trait;
use nvisy_core::Result;
use nvisy_core::nlp::RawNerSpan;
use nvisy_ontology::entity::EntityKind;
use nvisy_ontology::primitive::LanguageTag;
use uuid::Uuid;

/// Per-call request handed to a [`GlinerBackend`].
///
/// Bundles the call's text plus per-call inference knobs the
/// backend may honor (language hint, requested kinds, correlation
/// id for tracing).
#[derive(Debug, Clone)]
pub struct GlinerRequest<'a> {
    /// The text to scan.
    pub text: &'a str,
    /// Entity kinds the caller is asking the model to look for.
    /// Zero-shot — an empty list is meaningless and recognizers
    /// short-circuit before calling the backend.
    pub kinds: &'a [EntityKind],
    /// Optional language hint. Multilingual models may ignore;
    /// monolingual models may validate.
    pub language: Option<&'a LanguageTag>,
    /// Correlation UUID. Backends with a tracing channel propagate
    /// it (the Bento backend puts it on the `x-request-id`
    /// header); backends without one ignore.
    pub correlation_id: Option<Uuid>,
}

/// Zero-shot NER backend.
///
/// Stateless from the caller's perspective — long-lived
/// connections (HTTP keepalive, gRPC channels) belong inside the
/// impl, not in the trait. `Send + Sync + 'static` so the
/// backend lives behind `Arc<dyn _>` in
/// [`GlinerRecognizer`](crate::recognition::GlinerRecognizer).
#[async_trait]
pub trait GlinerBackend: Send + Sync + 'static {
    /// Scan `request.text` for the requested kinds and return raw
    /// model spans. The recognizer applies normalization on top.
    ///
    /// # Errors
    ///
    /// Returns a runtime error on transport failure, schema
    /// mismatch, or any backend-specific failure mode.
    async fn predict(&self, request: GlinerRequest<'_>) -> Result<Vec<RawNerSpan>>;

    /// Batch-predict. Default is a serial loop over
    /// [`predict`](Self::predict); backends with native batching
    /// override.
    ///
    /// # Errors
    ///
    /// Returns the first error encountered.
    async fn predict_batch(
        &self,
        requests: Vec<GlinerRequest<'_>>,
    ) -> Result<Vec<Vec<RawNerSpan>>> {
        let mut out = Vec::with_capacity(requests.len());
        for r in requests {
            out.push(self.predict(r).await?);
        }
        Ok(out)
    }
}
