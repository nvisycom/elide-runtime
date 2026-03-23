//! Named entity recognition operation.

//!
//! Runs at **phase 2**, after extraction. Drives language-model inference
//! to identify and classify named entities within extracted text.

use nvisy_codec::Span;
use nvisy_codec::handler::{TextData, TextSpanId};
use nvisy_core::{Error, ErrorKind, Result};
use nvisy_http::HttpClient;
use nvisy_ontology::entity::{Entity, EntityCategory, RecognitionMethod, TextLocation};
use nvisy_rig::agent::{
    AgentConfig, AgentProvider, DetectionConfig, KnownNerEntity, NerAgent, NerContext,
};
use tokio::sync::Mutex;

use crate::graph::NamedEntityRecognition as NamedEntityRecognitionCfg;
use crate::operation::Operation;
use crate::operation::context::SequentialContext;
use crate::operation::envelope::DetectedEntities;
use crate::pipeline::RuntimeConfig;

const TARGET: &str = "nvisy_engine::op::entity_recognition";

/// NER-based entity recognition. Wraps an [`NerAgent`] and carries
/// coreference state between spans via [`SequentialContext`].
pub struct EntityRecognition {
    agent: NerAgent,
    config: DetectionConfig,
    state: Mutex<Vec<KnownNerEntity>>,
}

impl EntityRecognition {
    fn build_agent(
        provider: &AgentProvider,
        config: AgentConfig,
        http_client: Option<HttpClient>,
    ) -> Result<NerAgent> {
        let agent = if let Some(client) = http_client {
            NerAgent::with_http_client(provider, config, client)
        } else {
            NerAgent::new(provider, config)
        }
        .map_err(|e| Error::validation(e.to_string(), "ner-agent"))?;
        Ok(agent)
    }

    /// Build from graph config and runtime dependencies.
    pub async fn new(
        cfg: &NamedEntityRecognitionCfg,
        runtime: &RuntimeConfig,
        http_client: &HttpClient,
    ) -> Result<Self> {
        let llm = runtime.llm.as_ref();
        let provider = llm
            .and_then(|s| s.provider.clone())
            .ok_or_else(|| Error::new(ErrorKind::Validation, "NER requires an LLM provider"))?;
        let agent_config = llm.and_then(|s| s.policy.clone()).unwrap_or_default();

        let agent = Self::build_agent(&provider, agent_config, Some(http_client.clone()))?;
        let config = DetectionConfig {
            entity_kinds: cfg.entity_kinds.clone(),
            confidence_threshold: cfg.confidence_threshold.unwrap_or(0.5),
            system_prompt: None,
        };

        Ok(Self {
            agent,
            config,
            state: Mutex::new(Vec::new()),
        })
    }

    pub(crate) async fn detect(
        &self,
        spans: Vec<Span<TextSpanId, TextData>>,
    ) -> Result<DetectedEntities> {
        tracing::debug!(target: TARGET, span_count = spans.len(), "running NER");
        let mut entities = Vec::new();

        for span in &spans {
            let known = self.state.lock().await.clone();
            let ctx = NerContext::with_known(span.data.as_str(), known);

            let ner_entities = self
                .agent
                .detect(&ctx, &self.config)
                .await
                .map_err(|e| Error::runtime(e.to_string(), "ner-agent", e.is_retryable()))?;

            for ne in &ner_entities {
                let category: EntityCategory = match ne.category {
                    Some(c) => c,
                    None => continue,
                };
                let entity_kind = match ne.entity_type {
                    Some(ek) => ek,
                    None => continue,
                };
                let confidence = ne.confidence.unwrap_or(0.0);
                if confidence < self.config.confidence_threshold {
                    continue;
                }

                let mut entity = Entity::new(
                    category,
                    entity_kind,
                    &ne.value,
                    RecognitionMethod::Ner,
                    confidence,
                );
                let loc = if let Some(offsets) = ne.resolve_offsets(&ctx) {
                    TextLocation {
                        start_offset: offsets.start,
                        end_offset: offsets.end,
                        element_id: Some(span.id.to_string()),
                        ..Default::default()
                    }
                } else {
                    TextLocation {
                        element_id: Some(span.id.to_string()),
                        ..Default::default()
                    }
                };
                entity = entity.with_location(loc.into());
                entities.push(entity.with_parent(&span.source));
            }

            let mut state = self.state.lock().await;
            let mut merge_ctx =
                NerContext::with_known(span.data.as_str(), std::mem::take(&mut *state));
            merge_ctx.merge(ner_entities);
            *state = merge_ctx.known_entities;
        }

        Ok(DetectedEntities(entities.into()))
    }

    pub(crate) async fn reset(&self) {
        self.state.lock().await.clear();
    }
}

impl Operation for EntityRecognition {
    type Input = SequentialContext<Vec<Span<TextSpanId, TextData>>>;
    type Output = SequentialContext<DetectedEntities>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.sequential_map(|spans| self.detect(spans)).await
    }
}
