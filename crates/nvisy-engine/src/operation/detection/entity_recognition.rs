//! Named entity recognition operation.
//!
//! Runs at **phase 2**, after extraction. Drives language-model inference
//! to identify and classify named entities within extracted text.

use nvisy_codec::Span;
use nvisy_codec::handler::TextData;
use nvisy_core::{Error, ErrorKind, Result};
use nvisy_ontology::entity::{Entities, TextLocation};
use nvisy_ontology::workflow::NerDetection;
use nvisy_provider::agent::{DetectionConfig, NerAgent};
use nvisy_provider::http::HttpClient;

use super::rebase_entities::RebaseEntities;
use crate::operation::{DocumentEnvelope, Operation};
use crate::pipeline::RuntimeConfig;

const TARGET: &str = "nvisy_engine::op::entity_recognition";

/// NER-based entity recognition. Wraps an [`NerAgent`] which manages
/// coreference state internally between successive text spans.
pub struct EntityRecognition {
    agent: NerAgent,
    config: DetectionConfig,
}

impl EntityRecognition {
    /// Build from graph config and runtime dependencies.
    pub async fn new(
        cfg: &NerDetection,
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

    async fn detect(&self, spans: &[Span<TextLocation, TextData>]) -> Result<Entities> {
        tracing::debug!(target: TARGET, span_count = spans.len(), "running NER");
        let mut entities = Entities::new();

        for span in spans {
            let detected: Entities = self
                .agent
                .detect_entities(span.data.as_str(), &self.config)
                .await
                .map_err(|e| Error::runtime(e.to_string(), "ner-agent", e.is_retryable()))?
                .into();

            entities.extend(detected.rebase_offsets(span));
        }

        Ok(entities)
    }
}

impl Operation for EntityRecognition {
    async fn execute(&self, envelope: &mut DocumentEnvelope) -> Result<()> {
        let spans = envelope.document.collect_text_spans().await;
        if !spans.is_empty() {
            let detected = self.detect(&spans).await?;
            tracing::debug!(
                target: TARGET,
                detected = detected.len(),
                "appending NER entities",
            );
            envelope.add_entities(detected).await;
        }
        self.agent.reset().await;
        Ok(())
    }
}
