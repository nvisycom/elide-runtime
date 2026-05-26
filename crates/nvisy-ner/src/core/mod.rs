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
use nvisy_ontology::document::Block;
use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::Text;

pub use self::context::Context;

/// Recognize entities in text.
///
/// Async because the realistic implementation paths — HTTP calls to
/// an externalized inference service, future LLM calls — need to
/// yield. Pure-CPU backends wrap the body in `async {}` cheaply.
///
/// The trait's wire boundary is `&str`: implementors take a string
/// and return entities with byte offsets into it. Block- and
/// document-level convenience helpers live on the trait as default
/// methods that delegate to [`recognize`].
///
/// [`recognize`]: Self::recognize
#[async_trait]
pub trait Backend: Send + Sync + 'static {
    /// Recognize entities in `text` under `ctx`.
    ///
    /// Returned entities have offsets into `text`. The caller maps
    /// those to source coordinates when storing on a block.
    async fn recognize(&self, text: &str, ctx: &Context) -> Result<Vec<Entity<Text>>>;

    /// Recognize entities in each of `texts` under one shared
    /// [`Context`], merging the per-text results.
    ///
    /// `texts` is assumed to be chunks of the same source (the
    /// language hint and entity allowlist apply uniformly). The
    /// default impl dispatches concurrently via `futures::join_all`
    /// and concatenates; backends with native batching override it.
    async fn recognize_batch(&self, texts: &[&str], ctx: &Context) -> Result<Vec<Entity<Text>>> {
        let pending: Vec<_> = texts.iter().map(|t| self.recognize(t, ctx)).collect();
        let results: Vec<Result<Vec<Entity<Text>>>> = futures::future::join_all(pending).await;
        let mut merged: Vec<Entity<Text>> = Vec::new();
        for r in results {
            merged.extend(r?);
        }
        Ok(merged)
    }

    /// Convenience: scan a single [`Block<Text>`] by passing its
    /// text to [`recognize`]. Returned entity offsets are relative
    /// to the block's text; the caller maps to source coordinates
    /// before storing on `block.entities`.
    ///
    /// [`recognize`]: Self::recognize
    async fn recognize_block(
        &self,
        block: &Block<Text>,
        ctx: &Context,
    ) -> Result<Vec<Entity<Text>>> {
        self.recognize(block.kind.text(), ctx).await
    }
}
