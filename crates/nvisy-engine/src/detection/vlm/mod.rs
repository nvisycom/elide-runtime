//! VLM-backed image recognizer wiring.
//!
//! The recognizer surface is [`VlmPipeline`] itself — this module
//! provides:
//!
//! - [`VlmDetection`]: the config bundle operators set in
//!   `[detection.vlm]`.
//! - [`build_pipeline`]: turn a `VlmDetection` into an
//!   [`Arc<VlmPipeline>`] ready to register on the image-side
//!   recognizer slot.
//! - `impl Recognizer<Modality = Image> for VlmPipeline`: lets the
//!   pipeline drop directly into the engine's image-modality
//!   dispatch path.
//! - `impl From<&VlmDetectionContext> for VlmScanInput`: maps the
//!   fat image detection context to the pipeline's per-call detect
//!   bundle.
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
use nvisy_ontology::primitive::Dimensions;

pub use self::params::{VlmDetectParams, VlmDetection, VlmVerifyParams};
use crate::detection::{Recognizer, VlmDetectionContext};

/// Per-call scan bundle for [`VlmPipeline`]. Pairs the agent-facing
/// [`VlmDetectContext`] with the encoded image bytes and pixel
/// dimensions. Built from a [`VlmDetectionContext`] via [`From`].
pub struct VlmScanInput {
    /// Agent-facing per-call config.
    pub ctx: VlmDetectContext,
    /// Encoded image bytes the VLM sees.
    pub image: bytes::Bytes,
    /// Pixel dimensions of the encoded image. Used by the agent to
    /// scale normalised VLM bboxes to pixel space.
    pub dims: Dimensions,
}

/// Build a configured [`VlmPipeline`] from a [`VlmDetection`]
/// config bundle.
///
/// At least one of `detect` or `verify` must be enabled — a
/// pipeline with neither would never produce or process entities.
///
/// # Errors
///
/// Returns an error if neither pass is enabled, or if any built
/// agent cannot be constructed.
pub fn build_pipeline(cfg: VlmDetection) -> Result<Arc<VlmPipeline>> {
    let detect_cfg = cfg.detect.filter(|d| d.enabled).map(|d| d.agent);
    let verify_cfg = cfg.verify.filter(|v| v.enabled).map(|v| v.agent);
    let pipeline = VlmPipeline::new(&cfg.provider, detect_cfg, verify_cfg)
        .map_err(|e| Error::runtime(e.to_string(), "vlm", false))?;
    Ok(Arc::new(pipeline))
}

#[async_trait::async_trait]
impl Recognizer for VlmPipeline {
    type Context = VlmScanInput;
    type Modality = Image;

    #[tracing::instrument(
        skip_all,
        fields(
            image_bytes = input.image.len(),
            width = input.dims.width,
            height = input.dims.height,
            correlation_id = input.ctx.correlation_id.as_ref().map(|id| id.to_string()),
        ),
    )]
    async fn run(&self, input: &VlmScanInput) -> Result<Vec<Entity<Image>>> {
        VlmPipeline::detect(self, &input.image, input.dims, &input.ctx)
            .await
            .map_err(|e| Error::runtime(e.to_string(), "vlm", false))
    }

    async fn reset(&self) {
        VlmPipeline::reset(self).await;
    }
}

impl From<&VlmDetectionContext> for VlmScanInput {
    fn from(ctx: &VlmDetectionContext) -> Self {
        Self {
            ctx: VlmDetectContext {
                entity_kinds: ctx.entities.clone().unwrap_or_default(),
                system_prompt: None,
                labels: ctx.labels.clone(),
                correlation_id: ctx.correlation_id,
            },
            image: ctx.image.clone(),
            dims: ctx.dims,
        }
    }
}
