//! [`DynRecognizer`]: object-safe bridge over [`Recognizer`].
//!
//! [`Recognizer`] uses an associated `Context` type so each
//! implementation declares exactly what it consumes. That makes the
//! trait itself impossible to store as `dyn Recognizer` (different
//! impls have different `Context`, so the trait isn't object-safe).
//!
//! `DynRecognizer` is the engine-internal shim that erases the
//! associated type: it takes the shared [`DetectionContext`] and
//! defers to each underlying [`Recognizer`] via that recognizer's
//! `From<&DetectionContext>` impl. A blanket impl wraps every
//! qualifying `Recognizer` automatically.
//!
//! [`Recognizer`]: nvisy_core::detection::Recognizer

use async_trait::async_trait;
use nvisy_core::Result;
use nvisy_core::detection::Recognizer;
use nvisy_ontology::entity::Entities;

use super::DetectionContext;

/// Object-safe recognizer surface used by [`DetectionEngine`].
///
/// Trait-object-friendly counterpart to [`Recognizer`]: every
/// qualifying recognizer (one whose `Context: From<&DetectionContext>`)
/// auto-implements this via a blanket impl.
///
/// [`DetectionEngine`]: super::DetectionEngine
/// [`Recognizer`]: nvisy_core::detection::Recognizer
#[async_trait]
pub trait DynRecognizer: Send + Sync {
    /// Detect entities, taking the fat [`DetectionContext`] and
    /// extracting the recognizer's typed slice internally.
    async fn run(&self, ctx: &DetectionContext) -> Result<Entities>;

    /// Reset per-document state. Forwarded to the underlying
    /// recognizer.
    async fn reset(&self);
}

#[async_trait]
impl<R> DynRecognizer for R
where
    R: Recognizer + Send + Sync,
    R::Context: for<'a> From<&'a DetectionContext> + Send + Sync,
{
    async fn run(&self, ctx: &DetectionContext) -> Result<Entities> {
        let typed = <R::Context>::from(ctx);
        Recognizer::run(self, &typed).await
    }

    async fn reset(&self) {
        Recognizer::reset(self).await;
    }
}
