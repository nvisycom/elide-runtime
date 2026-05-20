//! LLM-driven named entity recognition.
//!
//! Runs at **phase 2**, after extraction. Drives an LLM via
//! [`NerAgent`] to identify and classify named entities within
//! extracted text. Silently no-ops at the orchestrator level when no
//! LLM provider is configured — [`Self::new`] returns an error in
//! that case and the orchestrator skips the operation.
//!
//! For trait-driven NER over any [`nvisy_nlp::NerBackend`] —
//! typically an ONNX local model today — see
//! [`NerRecognition`](super::ner_recognition::NerRecognition).

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

const TARGET: &str = "nvisy_engine::op::llm_recognition";

/// LLM-backed entity recognition. Wraps an [`NerAgent`] which manages
/// coreference state internally between successive text spans.
pub struct LlmRecognition {
    agent: NerAgent,
    config: DetectionConfig,
}

impl LlmRecognition {
    /// Build from graph config and runtime dependencies.
    ///
    /// # Errors
    ///
    /// Returns a validation error if no LLM provider is configured.
    /// Callers (the orchestrator) treat this as "skip the operation",
    /// not as a fatal pipeline failure.
    pub async fn new(
        cfg: &NerDetection,
        runtime: &RuntimeConfig,
        http_client: &HttpClient,
    ) -> Result<Self> {
        let llm = runtime.llm.as_ref();
        let provider = llm
            .and_then(|s| s.provider.clone())
            .ok_or_else(|| {
                Error::new(ErrorKind::Validation, "LLM recognition requires an LLM provider")
            })?;
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
        tracing::debug!(target: TARGET, span_count = spans.len(), "running LLM recognition");
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

impl Operation for LlmRecognition {
    async fn execute(&self, envelope: &mut DocumentEnvelope) -> Result<()> {
        let spans = envelope.document.collect_text_spans().await;
        if !spans.is_empty() {
            let detected = self.detect(&spans).await?;
            tracing::debug!(
                target: TARGET,
                detected = detected.len(),
                "appending LLM-recognised entities",
            );
            envelope.add_entities(detected).await;
        }
        self.agent.reset().await;
        Ok(())
    }
}
