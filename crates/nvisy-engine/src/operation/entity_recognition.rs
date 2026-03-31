//! Named entity recognition operation.
//!
//! Runs at **phase 2**, after extraction. Drives language-model inference
//! to identify and classify named entities within extracted text.

use nvisy_codec::Span;
use nvisy_codec::handler::{TextData, TextSpanId};
use nvisy_core::{Error, ErrorKind, Result};
use nvisy_ontology::workflow::NamedEntityRecognition;
use nvisy_provider::agent::{AgentConfig, AgentProvider, DetectionConfig, NerAgent};
use nvisy_provider::http::HttpClient;

use crate::operation::Operation;
use crate::operation::context::SequentialContext;
use crate::operation::envelope::DetectedEntities;
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

    pub(crate) async fn detect(
        &self,
        spans: Vec<Span<TextSpanId, TextData>>,
    ) -> Result<DetectedEntities> {
        tracing::debug!(target: TARGET, span_count = spans.len(), "running NER");
        let mut entities = Vec::new();

        for span in &spans {
            let detected = self
                .agent
                .detect_entities(span.data.as_str(), &self.config)
                .await
                .map_err(|e| Error::runtime(e.to_string(), "ner-agent", e.is_retryable()))?;

            for mut entity in detected {
                entity = entity.with_parent(&span.source);

                if let Some(nvisy_ontology::entity::Location::Text(ref mut loc)) = entity.location {
                    loc.element_id = Some(span.id.to_string());
                }

                entities.push(entity);
            }
        }

        Ok(DetectedEntities(entities.into()))
    }

    /// Clear the agent's coreference state. Call between documents.
    pub(crate) async fn reset(&self) {
        self.agent.reset().await;
    }
}

impl Operation for EntityRecognitionOp {
    type Input = SequentialContext<Vec<Span<TextSpanId, TextData>>>;
    type Output = SequentialContext<DetectedEntities>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.sequential_map(|spans| self.detect(spans)).await
    }
}
