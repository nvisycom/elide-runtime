//! NER detection adapter wrapping [`NerAgent`] from `nvisy-rig`.
//!
//! Uses a [`SequentialContext`] so the orchestrator feeds one span at
//! a time, allowing the adapter to accumulate known entities between
//! spans for coreference resolution.

use serde::Deserialize;
use tokio::sync::Mutex;

use nvisy_codec::handler::{Span, TxtSpan};
use nvisy_ontology::entity::EntityKind;
use nvisy_core::Error;
use nvisy_ontology::entity::EntityCategory;
use nvisy_rig::{
    BaseAgentConfig, DetectionConfig, KnownNerEntity, NerAgent, NerContext, Provider,
};

use crate::{DetectionMethod, Entity, Location, TextLocation};
use crate::{SequentialContext, DetectionLayer, DetectionService};

fn default_confidence() -> f64 {
    0.5
}

/// Typed parameters for [`NerMethod`].
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
    pub provider: Option<Provider>,
    /// Optional agent config overrides.
    #[serde(skip)]
    pub agent_config: Option<BaseAgentConfig>,
}

/// Accumulated state between sequential span calls.
struct NerState {
    /// Known entities from prior detection calls (for coreference).
    known_entities: Vec<KnownNerEntity>,
}

/// NER detection method — thin adapter around [`NerAgent`].
///
/// Uses [`SequentialContext`]: the orchestrator feeds one span at a
/// time so the adapter can carry known-entity context between spans.
pub struct NerMethod {
    agent: NerAgent,
    config: DetectionConfig,
    state: Mutex<NerState>,
}

impl NerMethod {
    /// Create a new NER method from a pre-built agent and detection config.
    pub fn from_agent(agent: NerAgent, config: DetectionConfig) -> Self {
        Self {
            agent,
            config,
            state: Mutex::new(NerState {
                known_entities: Vec::new(),
            }),
        }
    }

    /// Clear accumulated state between documents.
    pub async fn reset(&self) {
        let mut state = self.state.lock().await;
        state.known_entities.clear();
    }
}

#[async_trait::async_trait]
impl DetectionLayer for NerMethod {
    type Params = NerMethodParams;

    async fn connect(params: Self::Params) -> Result<Self, Error> {
        let provider = params.provider.ok_or_else(|| {
            Error::validation("NerMethod requires a provider", "ner-method")
        })?;
        let agent_config = params.agent_config.unwrap_or_default();
        let agent = NerAgent::new(&provider, agent_config).map_err(|e| {
            Error::validation(e.to_string(), "ner-method")
        })?;
        let config = DetectionConfig {
            entity_kinds: params.entity_kinds,
            confidence_threshold: params.confidence_threshold,
            system_prompt: None,
        };
        Ok(Self::from_agent(agent, config))
    }
}

#[async_trait::async_trait]
impl DetectionService<TxtSpan, String> for NerMethod {
    type Context = SequentialContext;

    async fn detect(
        &self,
        spans: Vec<Span<TxtSpan, String>>,
    ) -> Result<Vec<Entity>, Error> {
        let mut entities = Vec::new();

        for span in &spans {
            // Build NER context with accumulated known entities.
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

            // Convert NerEntity → Entity with resolved offsets.
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

                // Resolve offsets within the current span text.
                if let Some(offsets) = ner_entity.resolve_offsets(&ctx) {
                    entity = entity.with_location(Location::Text(TextLocation {
                        start_offset: offsets.start,
                        end_offset: offsets.end,
                        element_id: Some(span.id.0.to_string()),
                        ..Default::default()
                    }));
                } else {
                    entity = entity.with_location(Location::Text(TextLocation {
                        element_id: Some(span.id.0.to_string()),
                        ..Default::default()
                    }));
                }

                entities.push(entity.with_parent(&span.source));
            }

            // Accumulate known entities for coreference across spans.
            let mut state = self.state.lock().await;
            let mut merge_ctx = NerContext::with_known(
                &span.data,
                std::mem::take(&mut state.known_entities),
            );
            merge_ctx.merge(ner_entities);
            state.known_entities = merge_ctx.known_entities;
        }

        Ok(entities)
    }
}
