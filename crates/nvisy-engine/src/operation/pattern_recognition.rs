//! Pattern recognition operation.
//!
//! Runs at **phase 2** alongside [`EntityRecognitionOp`]. Detects entities
//! using deterministic rules: regular expressions, checksums, and
//! dictionary lookups.
//!
//! [`EntityRecognitionOp`]: crate::operation::EntityRecognitionOp

use nvisy_codec::Span;
use nvisy_codec::handler::{TextData, TextSpanId};
use nvisy_core::Result;
use nvisy_ontology::entity::Entity;
use nvisy_ontology::workflow::PatternRecognition;

use crate::operation::Operation;
use crate::operation::context::ParallelContext;
use crate::operation::envelope::DetectedEntities;

const TARGET: &str = "nvisy_engine::op::pattern_recognition";

/// Holds either a borrowed reference to the global singleton or an
/// owned engine built from custom config.
enum EngineRef {
    Shared(&'static nvisy_pattern::PatternEngine),
    Owned(nvisy_pattern::PatternEngine),
}

impl std::ops::Deref for EngineRef {
    type Target = nvisy_pattern::PatternEngine;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Shared(e) => e,
            Self::Owned(e) => e,
        }
    }
}

/// Pattern-based entity recognition using regex and dictionary matching.
pub struct PatternRecognitionOp {
    engine: EngineRef,
}

impl PatternRecognitionOp {
    /// Create from graph config.
    ///
    /// When the config specifies pattern names or a confidence threshold,
    /// a custom engine is built. Otherwise the default singleton is used.
    pub fn new(cfg: &PatternRecognition) -> Self {
        let needs_custom = !cfg.patterns.is_empty() || cfg.confidence_threshold.is_some();

        let engine = if needs_custom {
            let mut builder = nvisy_pattern::PatternEngine::builder();
            if !cfg.patterns.is_empty() {
                let names: Vec<&str> = cfg.patterns.iter().map(String::as_str).collect();
                builder = builder.with_patterns(&names);
            }
            if let Some(threshold) = cfg.confidence_threshold {
                builder = builder.with_confidence_threshold(threshold);
            }
            EngineRef::Owned(builder.build().expect("pattern engine must compile"))
        } else {
            EngineRef::Shared(nvisy_pattern::PatternEngine::instance())
        };

        tracing::debug!(
            target: TARGET,
            custom = needs_custom,
            patterns = cfg.patterns.len(),
            "created pattern recognition operation",
        );

        Self { engine }
    }

    fn scan(&self, spans: &[Span<TextSpanId, TextData>]) -> Vec<Entity> {
        let scan_ctx = nvisy_pattern::ScanContext::default();
        let mut entities = Vec::new();

        for span in spans {
            let detected = self.engine.scan_entities(span.data.as_str(), &scan_ctx);

            for mut entity in detected {
                entity = entity.with_parent(&span.source);

                if let Some(nvisy_ontology::entity::Location::Text(ref mut loc)) = entity.location
                {
                    loc.element_id = Some(span.id.to_string());
                }

                entities.push(entity);
            }
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

impl Operation for PatternRecognitionOp {
    type Input = ParallelContext<Vec<Span<TextSpanId, TextData>>>;
    type Output = ParallelContext<DetectedEntities>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input
            .parallel_map(|spans| async move { Ok(DetectedEntities(self.scan(&spans).into())) })
            .await
    }
}
