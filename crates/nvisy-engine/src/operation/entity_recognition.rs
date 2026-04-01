//! Named entity recognition operation.
//!
//! Runs at **phase 2**, after extraction. Drives language-model inference
//! to identify and classify named entities within extracted text.

use nvisy_codec::Span;
use nvisy_codec::handler::{TextData, TextSpanId};
use nvisy_core::{Error, ErrorKind, Result};
use nvisy_ontology::entity::Entity;
use nvisy_ontology::workflow::NamedEntityRecognition;
use nvisy_provider::agent::{DetectionConfig, NerAgent};
use nvisy_provider::http::HttpClient;

use crate::operation::{DocumentEnvelope, Operation};
use crate::pipeline::RuntimeConfig;

const TARGET: &str = "nvisy_engine::op::entity_recognition";

/// NER-based entity recognition. Wraps an [`NerAgent`] which manages
/// coreference state internally between successive text spans.
pub struct EntityRecognitionOp {
    agent: NerAgent,
    config: DetectionConfig,
}

impl EntityRecognitionOp {
    /// Build from graph config and runtime dependencies.
    pub async fn new(
        cfg: &NamedEntityRecognition,
        runtime: &RuntimeConfig,
        http_client: &HttpClient,
    ) -> Result<Self> {
        let llm = runtime.llm.as_ref();
        let provider = llm
            .and_then(|s| s.provider.clone())
            .ok_or_else(|| Error::new(ErrorKind::Validation, "NER requires an LLM provider"))?;
        let agent_config = llm.and_then(|s| s.policy.clone()).unwrap_or_default();

        let agent = NerAgent::new(&provider, agent_config, Some(http_client.clone()))
            .map_err(|e| Error::validation(e.to_string(), "ner-agent"))?;
        let config = DetectionConfig {
            entity_kinds: cfg.entity_kinds.clone(),
            confidence_threshold: cfg.confidence_threshold.unwrap_or(0.5),
            system_prompt: None,
        };

        Ok(Self { agent, config })
    }

    async fn detect(&self, spans: &[Span<TextSpanId, TextData>]) -> Result<Vec<Entity>> {
        tracing::debug!(target: TARGET, span_count = spans.len(), "running NER");
        let mut entities = Vec::new();

        for span in spans {
            let detected = self
                .agent
                .detect_entities(span.data.as_str(), &self.config)
                .await
                .map_err(|e| Error::runtime(e.to_string(), "ner-agent", e.is_retryable()))?;

            for mut entity in detected {
                entity = entity.with_parent(&span.source);

                if let Some(nvisy_ontology::entity::Location::Text(ref mut loc)) = entity.location {
                    loc.span_index = Some(span.id.0);
                }

                entities.push(entity);
            }
        }

        Ok(entities)
    }
}

impl Operation for EntityRecognitionOp {
    async fn execute(&self, envelope: &mut DocumentEnvelope) -> Result<()> {
        let spans: Vec<_> = envelope.document.collect_text_spans().await;
        if !spans.is_empty() {
            let detected = self.detect(&spans).await?;
            tracing::debug!(
                target: TARGET,
                detected = detected.len(),
                "appending NER entities",
            );
            envelope.audit.entities.extend(detected);
        }
        self.agent.reset().await;
        Ok(())
    }
}
