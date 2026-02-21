//! Detection layer traits and processing-strategy markers.
//!
//! [`DetectionLayer`] handles construction and identity.
//! [`Detect`] provides span-level execution.  The associated
//! [`DetectionContext`] tells the orchestrator whether to batch
//! all spans or iterate one-by-one.

mod context;

pub use context::{DetectionContext, ParallelContext, SequentialContext};

use serde::de::DeserializeOwned;

use nvisy_codec::handler::Span;
use nvisy_core::Error;
use nvisy_core::path::ContentSource;

use crate::Entity;

/// Construction and identity for a detection layer.
///
/// A `DetectionLayer` knows how to build itself from typed parameters
/// and exposes a unique identifier.  Span-level execution is provided
/// by implementing [`Detect`] for each `(SpanId, SpanData)` pair the
/// layer supports.
#[async_trait::async_trait]
pub trait DetectionLayer: Sized + Send + Sync + 'static {
    /// Strongly-typed parameters for this layer.
    type Params: DeserializeOwned + Send;

    /// Validate parameters and construct a configured layer instance.
    async fn connect(params: Self::Params) -> Result<Self, Error>;
}

/// Span-level detection execution.
///
/// A layer implements `Detect<Id, Data>` for each handler span-type
/// combination it supports.  The associated [`Context`](Self::Context)
/// tells the orchestrator whether to batch all spans or iterate
/// one-by-one.
#[async_trait::async_trait]
pub trait Detect<Id, Data>: Send + Sync + 'static
where
    Id: Send + Sync + Clone + 'static,
    Data: Send + 'static,
{
    /// Processing strategy — [`ParallelContext`] or [`SequentialContext`].
    type Context: DetectionContext;

    /// Run detection over the given spans.
    ///
    /// - **`ParallelContext`** layers receive all spans at once.
    /// - **`SequentialContext`** layers receive one span per call; the
    ///   layer uses interior mutability to accumulate state.
    async fn detect(
        &self,
        spans: Vec<Span<Id, Data>>,
        source: &ContentSource,
    ) -> Result<Vec<Entity>, Error>;
}
