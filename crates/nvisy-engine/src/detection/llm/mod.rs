//! LLM-backed recognizer wiring.
//!
//! The recognizer surface is [`LlmNerPipeline`] itself — this module
//! provides:
//!
//! - [`LlmDetection`]: the config bundle operators set in
//!   `[detection.llm]`.
//! - [`build_pipeline`]: turn an `LlmDetection` into an
//!   [`Arc<LlmNerPipeline>`] ready to register on [`Recognizers`].
//! - `impl Recognizer for LlmNerPipeline`: lets the pipeline drop
//!   directly into the engine's dispatch path.
//! - `impl From<&DetectionContext> for LlmNerScanInput`: maps the
//!   fat engine context down to the pipeline's per-call config so
//!   the blanket [`DynTextRecognizer`] impl works without an extra
//!   wrapper.
//!
//! [`LlmNerPipeline`]: nvisy_agent::pipeline::LlmNerPipeline
//! [`Recognizers`]: super::Recognizers
//! [`DynTextRecognizer`]: super::DynTextRecognizer

mod params;

use std::sync::Arc;

use nvisy_agent::agent::LlmNerContext;
use nvisy_agent::pipeline::LlmNerPipeline;
use nvisy_codec::handler::TextData;
use nvisy_core::{Error, Result};
use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::Text;

pub use self::params::{DetectParams, LlmDetection, VerifyParams};
use crate::detection::{DetectionContext, Recognizer};

/// Per-call scan bundle for [`LlmNerPipeline`]. Pairs the
/// agent-facing [`LlmNerContext`] with the text to scan. Built
/// from a [`DetectionContext`] via [`From`].
pub struct LlmNerScanInput {
    /// Agent-facing per-call config.
    pub ctx: LlmNerContext,
    /// The text to scan.
    pub text: TextData,
}

/// Build a configured [`LlmNerPipeline`] from an [`LlmDetection`]
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
pub fn build_pipeline(cfg: LlmDetection) -> Result<Arc<LlmNerPipeline>> {
    let detect_cfg = cfg.detect.filter(|d| d.enabled).map(|d| d.agent);
    let verify_cfg = cfg.verify.filter(|v| v.enabled).map(|v| v.agent);
    let pipeline =
        LlmNerPipeline::new(&cfg.provider, detect_cfg, verify_cfg, cfg.unresolved_policy)
            .map_err(|e| Error::runtime(e.to_string(), "llm", false))?;
    Ok(Arc::new(pipeline))
}

#[async_trait::async_trait]
impl Recognizer for LlmNerPipeline {
    type Context = LlmNerScanInput;
    type Modality = Text;

    #[tracing::instrument(
        skip_all,
        fields(
            text_len = input.text.len(),
            correlation_id = input.ctx.correlation_id.as_ref().map(|id| id.to_string()),
        ),
    )]
    async fn run(&self, input: &LlmNerScanInput) -> Result<Vec<Entity<Text>>> {
        LlmNerPipeline::run(self, &input.text, &input.ctx)
            .await
            .map_err(|e| Error::runtime(e.to_string(), "llm", false))
    }

    /// Reset cumulative usage counters at document boundaries by
    /// delegating to [`LlmNerPipeline::reset`].
    async fn reset(&self) {
        LlmNerPipeline::reset(self).await;
    }
}

impl From<&DetectionContext> for LlmNerScanInput {
    fn from(ctx: &DetectionContext) -> Self {
        Self {
            ctx: LlmNerContext {
                entity_kinds: ctx.entities.clone().unwrap_or_default(),
                system_prompt: None,
                hints: ctx.hints.clone(),
                labels: ctx.labels.clone(),
                correlation_id: ctx.correlation_id,
            },
            text: ctx.text.clone(),
        }
    }
}
