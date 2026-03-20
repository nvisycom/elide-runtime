//! Pattern recognition operation.
//!
//! Runs at **phase 2** alongside [`EntityRecognition`]. Detects entities
//! using deterministic rules: regular expressions, checksums, dictionary
//! lookups, and structural heuristics.
//!
//! [`EntityRecognition`]: crate::operation::EntityRecognition

use nvisy_codec::handler::TextData;
use nvisy_codec::Span;
use nvisy_core::Result;
use nvisy_ontology::entity::{Entity, RecognitionMethod, TextLocation};

use crate::operation::context::{ParallelContext, SharedContext};
use crate::operation::envelope::DetectedEntities;
use crate::operation::Operation;

const TARGET: &str = "nvisy_engine::op::pattern_recognition";

/// Pattern-based entity recognition using regex and dictionary matching.
pub struct PatternRecognition {
    #[allow(dead_code)]
    shared: SharedContext,
}

impl PatternRecognition {
    /// Create a new pattern recognition operation.
    pub async fn new(shared: SharedContext) -> Result<Self> {
        Ok(Self { shared })
    }

    fn scan(spans: &[Span<usize, TextData>]) -> Vec<Entity> {
        let engine = nvisy_pattern::PatternEngine::instance();
        let scan_ctx = nvisy_pattern::ScanContext::default();
        let mut entities = Vec::new();

        for span in spans {
            let matches = engine.scan_text(span.data.as_str(), &scan_ctx);
            for m in matches {
                let entity = Entity::new(
                    m.category,
                    m.entity_kind,
                    &m.value,
                    RecognitionMethod::Regex,
                    m.confidence,
                )
                .with_location(
                    TextLocation {
                        start_offset: m.start,
                        end_offset: m.end,
                        element_id: Some(span.id.to_string()),
                        ..Default::default()
                    }
                    .into(),
                )
                .with_parent(&span.source);
                entities.push(entity);
            }
        }

        entities
    }

}

impl Operation for PatternRecognition {
    type Input = ParallelContext<Vec<Span<usize, TextData>>>;
    type Output = ParallelContext<DetectedEntities>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        tracing::debug!(target: TARGET, "running pattern detection");
        input
            .parallel_map(|spans| async move {
                let entities = Self::scan(&spans);
                tracing::debug!(target: TARGET, detected = entities.len(), "pattern scan complete");
                Ok(DetectedEntities(entities.into()))
            })
            .await
    }
}
