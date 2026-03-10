//! NER detection adapter wrapping [`NerAgent`] from `nvisy-rig`.
//!
//! Uses a [`SequentialContext`] so the orchestrator feeds one span at
//! a time, allowing the adapter to accumulate known entities between
//! spans for coreference resolution.

use nvisy_codec::Span;
use nvisy_codec::handler::TxtSpan;
use nvisy_core::{Error, Result};
use nvisy_http::HttpClient;
use nvisy_ontology::entity::{DetectionMethod, Entity, EntityCategory, EntityKind, TextLocation};
use nvisy_rig::agent::{
    AgentConfig, AgentProvider, DetectionConfig, KnownNerEntity, NerAgent, NerContext,
};
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::operation::{Operation, SequentialContext};

const TARGET: &str = "nvisy_engine::op::ner";

fn default_confidence() -> f64 {
    0.5
}

/// Typed parameters for [`Ner`].
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NerMethodParams {
    /// Entity kinds to detect (empty = all).
    #[serde(rename = "entityTypes", default)]
    pub entity_kinds: Vec<EntityKind>,
    /// Minimum confidence score for returned entities.
    #[serde(default = "default_confidence")]
    pub confidence_threshold: f64,
    /// Provider configuration for the NER agent.
    #[serde(skip)]
    pub provider: Option<AgentProvider>,
    /// Optional agent config overrides.
    #[serde(skip)]
    pub agent_config: Option<AgentConfig>,
    /// Pre-built HTTP client to share across providers.
    #[serde(skip)]
    pub http_client: Option<HttpClient>,
}

/// Accumulated state between sequential span calls.
struct NerState {
    /// Known entities from prior detection calls (for coreference).
    known_entities: Vec<KnownNerEntity>,
}

/// NER detection operation: thin adapter around [`NerAgent`].
///
/// Uses [`SequentialContext`]: the orchestrator feeds one span at a
/// time so the adapter can carry known-entity context between spans.
pub struct Ner {
    agent: NerAgent,
    config: DetectionConfig,
    state: Mutex<NerState>,
}

impl Ner {
    /// Create a new NER operation from a pre-built agent and detection config.
    pub fn from_agent(agent: NerAgent, config: DetectionConfig) -> Self {
        Self {
            agent,
            config,
            state: Mutex::new(NerState {
                known_entities: Vec::new(),
            }),
        }
    }

    /// Connect and build an NER operation from typed parameters.
    pub async fn connect(params: NerMethodParams) -> Result<Self> {
        let provider = params
            .provider
            .ok_or_else(|| Error::validation("Ner requires a provider", "ner-method"))?;
        let agent_config = params.agent_config.unwrap_or_default();
        let agent = if let Some(client) = params.http_client {
            NerAgent::with_http_client(&provider, agent_config, client)
        } else {
            NerAgent::new(&provider, agent_config)
        }
        .map_err(|e| Error::validation(e.to_string(), "ner-method"))?;
        let config = DetectionConfig {
            entity_kinds: params.entity_kinds,
            confidence_threshold: params.confidence_threshold,
            system_prompt: None,
        };
        Ok(Self::from_agent(agent, config))
    }

    /// Clear accumulated state between documents.
    pub async fn reset(&self) {
        let mut state = self.state.lock().await;
        state.known_entities.clear();
    }

    async fn detect(&self, spans: Vec<Span<TxtSpan, String>>) -> Result<Vec<Entity>> {
        tracing::debug!(target: TARGET, span_count = spans.len(), "running NER");
        let mut entities = Vec::new();

        for span in &spans {
            let known = {
                let state = self.state.lock().await;
                state.known_entities.clone()
            };
            let ctx = NerContext::with_known(&span.data, known);

            let ner_entities = self
                .agent
                .detect(&ctx, &self.config)
                .await
                .map_err(|e| Error::runtime(e.to_string(), "ner-agent", e.is_retryable()))?;

            for ner_entity in &ner_entities {
                let category: EntityCategory = match ner_entity.category {
                    Some(ref c) => c.clone(),
                    None => continue,
                };
                let entity_kind = match ner_entity.entity_type {
                    Some(ek) => ek,
                    None => continue,
                };
                let confidence = ner_entity.confidence.unwrap_or(0.0);
                if confidence < self.config.confidence_threshold {
                    continue;
                }

                let mut entity = Entity::new(
                    category,
                    entity_kind,
                    &ner_entity.value,
                    DetectionMethod::Ner,
                    confidence,
                );

                if let Some(offsets) = ner_entity.resolve_offsets(&ctx) {
                    entity = entity.with_location(
                        TextLocation {
                            start_offset: offsets.start,
                            end_offset: offsets.end,
                            element_id: Some(span.id.0.to_string()),
                            ..Default::default()
                        }
                        .into(),
                    );
                } else {
                    entity = entity.with_location(
                        TextLocation {
                            element_id: Some(span.id.0.to_string()),
                            ..Default::default()
                        }
                        .into(),
                    );
                }

                entities.push(entity.with_parent(&span.source));
            }

            let mut state = self.state.lock().await;
            let mut merge_ctx =
                NerContext::with_known(&span.data, std::mem::take(&mut state.known_entities));
            merge_ctx.merge(ner_entities);
            state.known_entities = merge_ctx.known_entities;
        }

        Ok(entities)
    }
}

impl Operation for Ner {
    type Input = SequentialContext<Vec<Span<TxtSpan, String>>>;
    type Output = SequentialContext<Vec<Entity>>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.sequential_map(|spans| self.detect(spans)).await
    }
}
