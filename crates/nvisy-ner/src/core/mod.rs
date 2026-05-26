//! Core NER contract: the [`Backend`] trait, the per-call
//! [`NerParams`] hints handed to backends, and the [`NerContext`]
//! engine-level call configuration.
//!
//! Backend implementations live in [`crate::backend`]; that module
//! also hosts the [`NerBackend`] config enum that dispatches to a
//! concrete backend.
//!
//! [`NerBackend`]: crate::backend::NerBackend

mod context;
mod params;

use async_trait::async_trait;
use nvisy_core::Result;
use nvisy_ontology::entity::Entities;

pub use self::context::{NerContext, NerContextBuilder, NerContextBuilderError};
pub use self::params::NerParams;

/// Recognize entities in text.
///
/// Async because the realistic implementation paths — HTTP calls to
/// an externalized inference service, future LLM calls — need to
/// yield. Pure-CPU backends wrap the body in `async {}` cheaply.
///
/// Implementors **must** provide [`recognize`]. The default
/// [`recognize_batch`] impl dispatches the inputs concurrently via
/// `futures::join_all` — for one-at-a-time HTTP backends this is
/// the right baseline. Backends with a native batch API (such as
/// the Bento backend) override it to issue a single round-trip.
///
/// Batch entries share a single [`NerParams`] — the typical caller
/// is post-tokenisation chunking from one document, so the language
/// hint and requested-kinds list apply uniformly. Mixed-context
/// inputs should be issued as separate batches.
///
/// [`recognize`]: Self::recognize
/// [`recognize_batch`]: Self::recognize_batch
#[async_trait]
pub trait Backend: Send + Sync + 'static {
    /// Recognize entities in `text` under `params`.
    async fn recognize(&self, text: &str, params: NerParams<'_>) -> Result<Entities>;

    /// Recognize entities in each of `texts` under one shared
    /// [`NerParams`].
    ///
    /// The returned vec is in the same order as `texts`. The
    /// default impl dispatches concurrently via
    /// `futures::join_all`; backends with native batching override
    /// it to issue one round-trip.
    ///
    /// [`recognize`]: Self::recognize
    async fn recognize_batch(
        &self,
        texts: &[&str],
        params: NerParams<'_>,
    ) -> Result<Vec<Entities>> {
        let pending: Vec<_> = texts.iter().map(|t| self.recognize(t, params)).collect();
        let results: Vec<Result<Entities>> = futures::future::join_all(pending).await;
        results.into_iter().collect()
    }
}
