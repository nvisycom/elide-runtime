//! Detection operation: dispatches to a [`DetectionEngine`].
//!
//! Runs at **phase 2**, after extraction. For each text span on the
//! envelope's document, builds a [`DetectionContext`] (carrying the
//! per-call hints from the workflow [`DetectionConfig`]) and runs
//! the attached `DetectionEngine`. Resulting entities are rebased
//! onto document coordinates and appended to the envelope.
//!
//! Per-document state (LLM coreference) is cleared by calling
//! [`DetectionEngine::reset`] at the end of each `execute`.
//!
//! [`DetectionConfig`]: crate::workflow::Detection

use std::sync::Arc;

use nvisy_codec::handler::TextData;
use nvisy_core::Result;

use super::{DocumentEnvelope, Operation};
use crate::detection::{DetectionContext, DetectionEngine, RebaseEntities};
use crate::workflow::Detection as DetectionConfig;

const TARGET: &str = "nvisy_engine::op::detection";

/// Wraps a shared [`DetectionEngine`] as a pipeline operation.
///
/// The engine is built once per run from the compiled workflow
/// `Detection` (see [`crate::detection::Detection::into_engine`])
/// and shared across every document in the run. The op is cheap
/// to build per-document — it only holds an `Arc` clone of the
/// engine and the workflow detection config.
pub(crate) struct Detection {
    engine: Arc<DetectionEngine>,
    cfg: DetectionConfig,
}

impl Detection {
    pub(crate) fn new(engine: Arc<DetectionEngine>, cfg: DetectionConfig) -> Self {
        Self { engine, cfg }
    }

    /// Build a per-span DetectionContext from the workflow config
    /// plus the run-level correlation id.
    ///
    /// Shared cross-recognizer hints (entity_kinds,
    /// confidence_threshold) flow from the workflow config's
    /// `params` field into DetectionContext. Every recognizer
    /// honors them: NER consumes them via nvisy_nlp::Context, LLM
    /// translates them into its rig DetectionConfig per call,
    /// pattern applies them as post-filters. Pattern-engine
    /// settings (patterns, filter, default threshold) are baked
    /// into the PatternRecognizer at construction time.
    fn build_context(&self, text: TextData, run_id: uuid::Uuid) -> DetectionContext {
        let mut ctx = DetectionContext::new(text);
        ctx.correlation_id = Some(run_id);
        if let Some(ref params) = self.cfg.params {
            if !params.entity_kinds.is_empty() {
                ctx.entities = Some(params.entity_kinds.clone());
            }
            if let Some(threshold) = params.confidence_threshold {
                ctx.score_threshold = Some(threshold);
            }
        }
        ctx
    }
}

impl Operation for Detection {
    async fn execute(&self, envelope: &mut DocumentEnvelope) -> Result<()> {
        let spans = envelope.document.collect_text_spans().await;
        if spans.is_empty() {
            return Ok(());
        }

        let run_id = envelope.shared.run_id;
        let mut all = nvisy_ontology::entity::Entities::new();
        for span in &spans {
            let ctx = self.build_context(span.data.clone(), run_id);
            let detected = self
                .engine
                .run(ctx)
                .await
                .map_err(nvisy_core::Error::from)?;
            all.extend(detected.rebase_offsets(span));
        }

        tracing::debug!(
            target: TARGET,
            detected = all.len(),
            spans = spans.len(),
            "appending detected entities",
        );
        envelope.add_entities(all).await;

        // Coreference state is per-document; reset between
        // documents so the next call starts clean.
        self.engine.reset().await;
        Ok(())
    }
}
