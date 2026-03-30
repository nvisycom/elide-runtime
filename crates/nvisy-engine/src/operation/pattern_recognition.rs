//! Pattern recognition operation.
//!
//! Runs at **phase 2** alongside [`EntityRecognition`]. Detects entities
//! using deterministic rules: regular expressions, checksums, and
//! dictionary lookups.
//!
//! [`EntityRecognition`]: crate::operation::EntityRecognition

use nvisy_codec::Span;
use nvisy_codec::handler::{TextData, TextSpanId};
use nvisy_core::Result;
use nvisy_ontology::entity::Entity;
use nvisy_ontology::workflow::PatternRecognition as PatternRecognitionCfg;

use crate::operation::Operation;
use crate::operation::context::ParallelContext;
use crate::operation::envelope::DetectedEntities;

const TARGET: &str = "nvisy_engine::op::pattern_recognition";

/// Pattern-based entity recognition using regex and dictionary matching.
pub struct PatternRecognition {
    engine: &'static nvisy_pattern::PatternEngine,
    contextual_analysis: bool,
}

impl PatternRecognition {
    /// Create from graph config.
    ///
    /// When the config specifies pattern names or a confidence threshold,
    /// a custom engine is built. Otherwise the default singleton is used.
    pub fn new(cfg: &PatternRecognitionCfg) -> Self {
        let needs_custom = !cfg.patterns.is_empty() || cfg.confidence_threshold.is_some();

        let engine = if needs_custom {
            // A custom engine lives in a leaked Box so it has 'static
            // lifetime matching the singleton path. This is acceptable
            // because engine instances are created once per pipeline run.
            let mut builder = nvisy_pattern::PatternEngine::builder();
            if !cfg.patterns.is_empty() {
                let names: Vec<&str> = cfg.patterns.iter().map(String::as_str).collect();
                builder = builder.with_patterns(&names);
            }
            if let Some(threshold) = cfg.confidence_threshold {
                builder = builder.with_confidence_threshold(threshold);
            }
            let engine = builder.build().expect("pattern engine must compile");
            Box::leak(Box::new(engine))
        } else {
            nvisy_pattern::PatternEngine::instance()
        };

        tracing::debug!(
            target: TARGET,
            custom = needs_custom,
            patterns = cfg.patterns.len(),
            contextual = cfg.contextual_analysis,
            "created pattern recognition operation",
        );

        Self {
            engine,
            contextual_analysis: cfg.contextual_analysis,
        }
    }

    fn scan(&self, spans: &[Span<TextSpanId, TextData>]) -> Vec<Entity> {
        let scan_ctx = nvisy_pattern::ScanContext::default();
        let mut entities = Vec::new();

        for span in spans {
            let detected = self.engine.scan_entities(span.data.as_str(), &scan_ctx);

            for mut entity in detected {
                entity = entity.with_parent(&span.source);

                // Fill in span-level element_id on the location.
                if let Some(nvisy_ontology::entity::Location::Text(ref mut loc)) = entity.location {
                    loc.element_id = Some(span.id.to_string());
                }

                entities.push(entity);
            }
        }

        entities
    }
}

impl Operation for PatternRecognition {
    type Input = ParallelContext<Vec<Span<TextSpanId, TextData>>>;
    type Output = ParallelContext<DetectedEntities>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input
            .parallel_map(|spans| async move {
                let entities = self.scan(&spans);
                tracing::info!(
                    target: TARGET,
                    detected = entities.len(),
                    spans = spans.len(),
                    "pattern scan complete",
                );
                Ok(DetectedEntities(entities.into()))
            })
            .await
    }
}
