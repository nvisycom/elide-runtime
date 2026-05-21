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
//! [`DetectionConfig`]: nvisy_ontology::workflow::Detection

use std::sync::Arc;

use nvisy_core::Result;
use nvisy_detection::{DetectionContext, DetectionEngine, RebaseEntities};
use nvisy_ontology::workflow::Detection as DetectionConfig;

use super::{DocumentEnvelope, Operation};

const TARGET: &str = "nvisy_engine::op::detection";

/// Wraps a shared [`DetectionEngine`] as a pipeline operation.
///
/// The engine is supplied by the caller (constructed externally
/// with whatever recognizers the user wants). The op is cheap to
/// build per-document — it only holds an `Arc` clone of the engine
/// and the workflow detection config.
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
    /// NER-side hints (entity_kinds, confidence_threshold) and the
    /// correlation_id flow through DetectionContext to every
    /// recognizer (NER consumes them via nvisy_nlp::Context, LLM
    /// translates them into its rig DetectionConfig per call,
    /// pattern applies them as post-filters). Pattern-engine
    /// settings (patterns, filter, default threshold) are baked
    /// into the PatternRecognizer at construction time.
    fn build_context<'a>(&self, text: &'a str, run_id: uuid::Uuid) -> DetectionContext<'a> {
        let mut ctx = DetectionContext::new(text);
        ctx.correlation_id = Some(run_id);
        if let Some(ref ner) = self.cfg.ner {
            if !ner.entity_kinds.is_empty() {
                ctx.entities = Some(ner.entity_kinds.clone());
            }
            if let Some(threshold) = ner.confidence_threshold {
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
            let ctx = self.build_context(span.data.as_str(), run_id);
            let detected = self
                .engine
                .run(&ctx)
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
