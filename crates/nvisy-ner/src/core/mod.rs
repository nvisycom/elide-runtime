//! Core NER contract: the [`Backend`] trait and the per-call
//! [`Context`] hints handed to backends and the engine.
//!
//! Backend implementations live in [`crate::backend`]; that module
//! also hosts the [`NerBackend`] config enum that dispatches to a
//! concrete backend.
//!
//! [`NerBackend`]: crate::backend::NerBackend

mod context;

use async_trait::async_trait;
use nvisy_core::Result;
use nvisy_ontology::entity::Entities;
use nvisy_ontology::modality::Text;

pub use self::context::Context;

/// Recognize entities in text.
///
/// Async because the realistic implementation paths — HTTP calls to
/// an externalized inference service, future LLM calls — need to
/// yield. Pure-CPU backends wrap the body in `async {}` cheaply.
///
/// Implementors **must** provide [`recognize`]. The default
/// [`recognize_batch`] impl dispatches the inputs concurrently via
/// `futures::join_all` and concatenates the per-text results into a
/// single [`Entities`]. Backends with a native batch API (such as
/// the Bento backend) override it to issue one round-trip and
/// merge server-side.
///
/// Batch entries share a single [`Context`] and are **assumed
/// to come from the same source** — the typical caller is
/// post-tokenisation chunking from one document, so the language
/// hint and entity allowlist apply uniformly and the per-text
/// entity offsets can be merged into one [`Entities`] without
/// further bookkeeping. Mixed-source inputs should be issued as
/// separate batches.
///
/// Per-text offsets are returned as-is — if the caller needs them
/// rebased onto a containing document, the caller knows the
/// per-chunk text offsets and is responsible for that rebase.
///
/// [`recognize`]: Self::recognize
/// [`recognize_batch`]: Self::recognize_batch
#[async_trait]
pub trait Backend: Send + Sync + 'static {
    /// Recognize entities in `text` under `ctx`.
    async fn recognize(&self, text: &str, ctx: &Context) -> Result<Entities<Text>>;

    /// Recognize entities in each of `texts` under one shared
    /// [`Context`], merging the per-text results into one
    /// [`Entities`].
    ///
    /// `texts` is assumed to be chunks of the same source (see the
    /// trait-level docs). The default impl dispatches concurrently
    /// via `futures::join_all` and concatenates; backends with
    /// native batching override it.
    ///
    /// [`recognize`]: Self::recognize
    async fn recognize_batch(&self, texts: &[&str], ctx: &Context) -> Result<Entities<Text>> {
        let pending: Vec<_> = texts.iter().map(|t| self.recognize(t, ctx)).collect();
        let results: Vec<Result<Entities<Text>>> = futures::future::join_all(pending).await;
        let mut merged = Entities::new();
        for r in results {
            merged.0.extend(r?.0);
        }
        Ok(merged)
    }
}
