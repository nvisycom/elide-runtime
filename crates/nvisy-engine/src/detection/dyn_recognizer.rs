//! Object-safe modality-typed shims over [`Recognizer`].
//!
//! [`Recognizer`] uses an associated `Context` type so each
//! implementation declares exactly what it consumes. That makes the
//! trait itself impossible to store as `dyn Recognizer` (different
//! impls have different `Context`, so the trait isn't object-safe).
//!
//! `DynTextRecognizer` and `DynImageRecognizer` are the
//! engine-internal shims that erase the associated type: each takes
//! the modality's fat [`DetectionContext`] / [`VlmDetectionContext`]
//! and defers to the underlying [`Recognizer`] via that recognizer's
//! `From<&...>` impl. Blanket impls wrap every qualifying
//! `Recognizer` automatically.
//!
//! [`Recognizer`]: crate::detection::Recognizer

use nvisy_core::Result;
use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::{Image, Text};

use super::{DetectionContext, VlmDetectionContext};
use crate::detection::Recognizer;

/// Object-safe text-modality recognizer surface used by
/// [`DetectionEngine`].
///
/// Auto-implemented for every [`Recognizer`] whose `Modality` is
/// [`Text`] and whose `Context: From<&DetectionContext>`.
///
/// [`DetectionEngine`]: super::DetectionEngine
/// [`Recognizer`]: crate::detection::Recognizer
#[async_trait::async_trait]
pub trait DynTextRecognizer: Send + Sync {
    /// Detect entities, taking the fat [`DetectionContext`] and
    /// extracting the recognizer's typed slice internally.
    async fn run(&self, ctx: &DetectionContext) -> Result<Vec<Entity<Text>>>;

    /// Reset per-document state. Forwarded to the underlying
    /// recognizer.
    async fn reset(&self);
}

#[async_trait::async_trait]
impl<R> DynTextRecognizer for R
where
    R: Recognizer<Modality = Text> + Send + Sync,
    R::Context: for<'a> From<&'a DetectionContext> + Send + Sync,
{
    async fn run(&self, ctx: &DetectionContext) -> Result<Vec<Entity<Text>>> {
        let typed = <R::Context>::from(ctx);
        Recognizer::run(self, &typed).await
    }

    async fn reset(&self) {
        Recognizer::reset(self).await;
    }
}

/// Object-safe image-modality recognizer surface used by
/// [`DetectionEngine`] when scanning image blocks.
///
/// Auto-implemented for every [`Recognizer`] whose `Modality` is
/// [`Image`] and whose `Context: From<&VlmDetectionContext>`.
///
/// [`DetectionEngine`]: super::DetectionEngine
/// [`Recognizer`]: crate::detection::Recognizer
#[async_trait::async_trait]
pub trait DynImageRecognizer: Send + Sync {
    /// Detect entities, taking the fat [`VlmDetectionContext`] and
    /// extracting the recognizer's typed slice internally.
    async fn run(&self, ctx: &VlmDetectionContext) -> Result<Vec<Entity<Image>>>;

    /// Reset per-document state. Forwarded to the underlying
    /// recognizer.
    async fn reset(&self);
}

#[async_trait::async_trait]
impl<R> DynImageRecognizer for R
where
    R: Recognizer<Modality = Image> + Send + Sync,
    R::Context: for<'a> From<&'a VlmDetectionContext> + Send + Sync,
{
    async fn run(&self, ctx: &VlmDetectionContext) -> Result<Vec<Entity<Image>>> {
        let typed = <R::Context>::from(ctx);
        Recognizer::run(self, &typed).await
    }

    async fn reset(&self) {
        Recognizer::reset(self).await;
    }
}
