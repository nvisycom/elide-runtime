//! Detection operation: dispatches to a [`DetectionEngine`].
//!
//! Runs at **phase 2**, after extraction. For each text span on the
//! envelope's document, builds a [`DetectionContext`] and runs the
//! attached `DetectionEngine`. Resulting entities are rebased onto
//! document coordinates and appended to the envelope.
//!
//! Per-document state (LLM coreference) is cleared by calling
//! [`DetectionEngine::reset`] at the end of each `execute`.

use std::sync::Arc;

use nvisy_core::Result;
use nvisy_detection::{DetectionContext, DetectionEngine, RebaseEntities};

use super::{DocumentEnvelope, Operation};

const TARGET: &str = "nvisy_engine::op::detection";

/// Wraps a shared [`DetectionEngine`] as a pipeline operation.
///
/// The engine is supplied by the caller (constructed externally
/// with whatever recognizers the user wants). The op is cheap to
/// build per-document — it only holds an `Arc` clone of the
/// engine.
pub(crate) struct Detection {
    engine: Arc<DetectionEngine>,
}

impl Detection {
    pub(crate) fn new(engine: Arc<DetectionEngine>) -> Self {
        Self { engine }
    }
}

impl Operation for Detection {
    async fn execute(&self, envelope: &mut DocumentEnvelope) -> Result<()> {
        let spans = envelope.document.collect_text_spans().await;
        if spans.is_empty() {
            return Ok(());
        }

        let mut all = nvisy_ontology::entity::Entities::new();
        for span in &spans {
            let ctx = DetectionContext::new(span.data.as_str());
            let detected = self
                .engine
                .detect(&ctx)
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
