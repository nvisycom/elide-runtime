//! LLM-backed text recognizer wiring.
//!
//! This module provides:
//!
//! - [`LlmDetection`]: the config bundle operators set in
//!   `[detection.llm]`.
//! - [`LlmRecognizer`]: newtype wrapping an [`LlmNerPipeline`] that
//!   impls [`TextRecognizer`] so it drops into the engine's dispatch
//!   path.
//! - [`build_recognizer`]: turn an `LlmDetection` into an
//!   [`Arc<LlmRecognizer>`] ready to register on [`DetectionEngine`].
//!
//! [`LlmNerPipeline`]: nvisy_agent::pipeline::LlmNerPipeline
//! [`DetectionEngine`]: super::DetectionEngine

mod params;

use std::sync::Arc;

use nvisy_agent::agent::LlmNerContext;
use nvisy_agent::pipeline::LlmNerPipeline;
use nvisy_core::{Error, Result};
use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::Text;

pub use self::params::{DetectParams, LlmDetection, VerifyParams};
use crate::detection::TextDetectionContext;
use crate::detection::recognizer::TextRecognizer;

/// Engine-side wrapper around [`LlmNerPipeline`].
///
/// Newtype so we can impl [`TextRecognizer`] on it without violating
/// the orphan rule. Behaviourally identical to the pipeline — every
/// method just forwards.
pub struct LlmRecognizer {
    inner: Arc<LlmNerPipeline>,
}

impl LlmRecognizer {
    /// Wrap a pre-built pipeline.
    pub fn from_inner(inner: Arc<LlmNerPipeline>) -> Self {
        Self { inner }
    }
}

/// Build a configured [`LlmRecognizer`] from an [`LlmDetection`]
/// config bundle.
///
/// Both the `[detect]` and `[verify]` sub-tables follow the
/// presence-and-flag pattern:
/// - sub-table absent → disabled
/// - sub-table present with `enabled = false` → disabled
/// - sub-table present (with the default `enabled = true` or
///   explicit `true`) → enabled
///
/// At least one of the two must be enabled — a recognizer with
/// neither would never produce or process entities.
///
/// # Errors
///
/// Returns an error if neither pass is enabled, or if any built
/// agent cannot be constructed (bad provider, invalid config).
pub fn build_recognizer(cfg: LlmDetection) -> Result<Arc<LlmRecognizer>> {
    let detect_cfg = cfg.detect.filter(|d| d.enabled).map(|d| d.agent);
    let verify_cfg = cfg.verify.filter(|v| v.enabled).map(|v| v.agent);
    let pipeline =
        LlmNerPipeline::new(&cfg.provider, detect_cfg, verify_cfg, cfg.unresolved_policy)
            .map_err(|e| Error::runtime(e.to_string(), "llm", false))?;
    Ok(Arc::new(LlmRecognizer::from_inner(Arc::new(pipeline))))
}

#[async_trait::async_trait]
impl TextRecognizer for LlmRecognizer {
    #[tracing::instrument(
        skip_all,
        fields(
            text_len = ctx.text.len(),
            correlation_id = ctx.correlation_id.as_ref().map(|id| id.to_string()),
        ),
    )]
    async fn recognize(&self, ctx: &TextDetectionContext) -> Result<Vec<Entity<Text>>> {
        let llm_ctx = LlmNerContext {
            entity_kinds: ctx.entities.clone().unwrap_or_default(),
            system_prompt: None,
            hints: ctx.hints.clone(),
            labels: ctx.labels.clone(),
            correlation_id: ctx.correlation_id,
        };
        self.inner
            .run(&ctx.text, &llm_ctx)
            .await
            .map_err(|e| Error::runtime(e.to_string(), "llm", false))
    }

    /// Reset cumulative usage counters at document boundaries.
    async fn reset(&self) {
        self.inner.reset().await;
    }
}
