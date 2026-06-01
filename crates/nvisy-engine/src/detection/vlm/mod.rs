//! VLM-backed image recognizer wiring.
//!
//! This module provides:
//!
//! - [`VlmDetection`]: the config bundle operators set in
//!   `[detection.vlm]`.
//! - [`VlmRecognizer`]: newtype wrapping a [`VlmPipeline`] that impls
//!   [`ImageRecognizer`] so it drops into the engine's image-modality
//!   dispatch path.
//! - [`build_recognizer`]: turn a `VlmDetection` into an
//!   [`Arc<VlmRecognizer>`] ready to register on the image-side slot.
//!
//! Verify-side runs as a separate post-detection phase orchestrated
//! by [`DetectionEngine`] — it is not part of the recognizer impl.
//!
//! [`VlmPipeline`]: nvisy_agent::pipeline::VlmPipeline
//! [`DetectionEngine`]: super::DetectionEngine

mod params;

use std::sync::Arc;

use nvisy_agent::agent::VlmDetectContext;
use nvisy_agent::pipeline::VlmPipeline;
use nvisy_core::{Error, Result};
use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::Image;

pub use self::params::{VlmDetectParams, VlmDetection, VlmVerifyParams};
use crate::detection::ImageDetectionContext;
use crate::detection::recognizer::ImageRecognizer;

/// Engine-side wrapper around [`VlmPipeline`].
///
/// Newtype so we can impl [`ImageRecognizer`] on it without violating
/// the orphan rule. Behaviourally identical to the pipeline.
pub struct VlmRecognizer {
    inner: Arc<VlmPipeline>,
}

impl VlmRecognizer {
    /// Wrap a pre-built pipeline.
    pub fn from_inner(inner: Arc<VlmPipeline>) -> Self {
        Self { inner }
    }
}

/// Build a configured [`VlmRecognizer`] from a [`VlmDetection`]
/// config bundle.
///
/// At least one of `detect` or `verify` must be enabled — a
/// pipeline with neither would never produce or process entities.
///
/// # Errors
///
/// Returns an error if neither pass is enabled, or if any built
/// agent cannot be constructed.
pub fn build_recognizer(cfg: VlmDetection) -> Result<Arc<VlmRecognizer>> {
    let detect_cfg = cfg.detect.filter(|d| d.enabled).map(|d| d.agent);
    let verify_cfg = cfg.verify.filter(|v| v.enabled).map(|v| v.agent);
    let pipeline = VlmPipeline::new(&cfg.provider, detect_cfg, verify_cfg)
        .map_err(|e| Error::runtime(e.to_string(), "vlm", false))?;
    Ok(Arc::new(VlmRecognizer::from_inner(Arc::new(pipeline))))
}

#[async_trait::async_trait]
impl ImageRecognizer for VlmRecognizer {
    #[tracing::instrument(
        skip_all,
        fields(
            image_bytes = ctx.image.len(),
            width = ctx.dims.width,
            height = ctx.dims.height,
            correlation_id = ctx.correlation_id.as_ref().map(|id| id.to_string()),
        ),
    )]
    async fn recognize(&self, ctx: &ImageDetectionContext) -> Result<Vec<Entity<Image>>> {
        let vlm_ctx = VlmDetectContext {
            entity_kinds: ctx.entities.clone().unwrap_or_default(),
            system_prompt: None,
            labels: ctx.labels.clone(),
            correlation_id: ctx.correlation_id,
        };
        self.inner
            .detect(&ctx.image, ctx.dims, &vlm_ctx)
            .await
            .map_err(|e| Error::runtime(e.to_string(), "vlm", false))
    }

    async fn reset(&self) {
        self.inner.reset().await;
    }
}
