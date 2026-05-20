//! Pattern recognition operation.
//!
//! Runs at **phase 2** alongside [`EntityRecognition`]. Detects entities
//! using deterministic rules: regular expressions, checksums, and
//! dictionary lookups.
//!
//! [`EntityRecognition`]: crate::operation::EntityRecognition

use nvisy_codec::Span;
use nvisy_codec::handler::TextData;
use nvisy_core::Result;
use nvisy_ontology::entity::{Entities, TextLocation};
use nvisy_ontology::workflow::PatternDetection;

use super::pattern_engine::PatternEngineRef;
use super::rebase_entities::RebaseEntities;
use crate::operation::{DocumentEnvelope, Operation};

const TARGET: &str = "nvisy_engine::op::pattern_recognition";

/// Pattern-based entity recognition using regex and dictionary matching.
pub struct PatternRecognition {
    engine: PatternEngineRef,
}

impl PatternRecognition {
    /// Create from graph config. Resolution between the shared default
    /// engine and a custom-built one lives on [`PatternEngineRef::new`].
    pub fn new(cfg: &PatternDetection) -> Self {
        let engine = PatternEngineRef::new(cfg);
        tracing::debug!(
            target: TARGET,
            patterns = cfg.patterns.len(),
            "created pattern recognition operation",
        );
        Self { engine }
    }

    fn scan(&self, spans: &[Span<TextLocation, TextData>]) -> Entities {
        let scan_ctx = nvisy_pattern::ScanContext::default();
        let mut entities = Entities::new();

        for span in spans {
            let detected: Entities = self
                .engine
                .scan_entities(span.data.as_str(), &scan_ctx)
                .into();
            entities.extend(detected.rebase_offsets(span));
        }

        tracing::info!(
            target: TARGET,
            detected = entities.len(),
            spans = spans.len(),
            "pattern scan complete",
        );

        entities
    }
}

impl Operation for PatternRecognition {
    async fn execute(&self, envelope: &mut DocumentEnvelope) -> Result<()> {
        let spans = envelope.document.collect_text_spans().await;
        if !spans.is_empty() {
            let detected = self.scan(&spans);
            tracing::debug!(
                target: TARGET,
                detected = detected.len(),
                "appending pattern entities",
            );
            envelope.add_entities(detected).await;
        }
        Ok(())
    }
}
