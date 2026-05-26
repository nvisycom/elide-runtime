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
//! [`Recognizer`]: crate::detection::Recognizer

use async_trait::async_trait;
use nvisy_core::Result;
use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::Text;

use super::DetectionContext;
use crate::detection::Recognizer;

/// Object-safe recognizer surface used by [`DetectionEngine`].
///
/// Trait-object-friendly counterpart to [`Recognizer`]: every
/// qualifying recognizer (one whose `Context: From<&DetectionContext>`)
/// auto-implements this via a blanket impl.
///
/// All built-in recognizers operate on text today; non-text
/// recognizers (image CV, audio) will have their own engines.
///
/// [`DetectionEngine`]: super::DetectionEngine
/// [`Recognizer`]: crate::detection::Recognizer
#[async_trait]
pub trait DynRecognizer: Send + Sync {
    /// Detect entities, taking the fat [`DetectionContext`] and
    /// extracting the recognizer's typed slice internally.
    async fn run(&self, ctx: &DetectionContext) -> Result<Vec<Entity<Text>>>;

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
    async fn run(&self, ctx: &DetectionContext) -> Result<Vec<Entity<Text>>> {
        let typed = <R::Context>::from(ctx);
        Recognizer::run(self, &ctx.text, &typed).await
    }

    async fn reset(&self) {
        Recognizer::reset(self).await;
    }
}
