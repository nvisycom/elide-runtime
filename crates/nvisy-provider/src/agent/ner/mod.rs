//! Named Entity Recognition (NER) agent for textual PII/entity detection.
//!
//! [`NerAgent`] wraps a [`BaseAgent`](crate::backend::BaseAgent) with
//! NER-specific prompts. It is a pure LLM agent (no tools) that analyses
//! text and returns structured entity detections.

mod context;
mod output;
mod prompt;

use nvisy_core::Result;
use nvisy_ontology::entity::{
    Entity, EntityCategory, ModelInfo, ModelKind, RecognitionMethod, TextLocation,
};
use tokio::sync::Mutex;
use uuid::Uuid;

pub use self::context::NerContext;
pub use self::output::{KnownNerEntity, NerEntities, NerEntity, ResolvedOffsets};
use self::prompt::{NER_SYSTEM_PROMPT, NerPromptBuilder};
use super::base::UsageTracker;
use super::{AgentConfig, AgentProvider, BaseAgent, DetectionConfig};
use crate::http::HttpClient;

const TARGET: &str = "nvisy_provider::agent::ner";

/// Agent for textual PII/entity detection using LLM-based NER.
///
/// # Workflow
///
/// 1. Caller passes a [`NerContext`] and a [`DetectionConfig`] to
///    [`detect`](Self::detect).
/// 2. The agent builds a user prompt via `NerPromptBuilder` that
///    specifies entity types, confidence thresholds, and known entities.
/// 3. Structured output is parsed into `Vec<NerEntity>`.
pub struct NerAgent {
    base: BaseAgent,
    state: Mutex<Vec<KnownNerEntity>>,
}

impl NerAgent {
    /// Create a new NER agent.
    ///
    /// Pass an [`HttpClient`] to share a connection pool with other
    /// services; otherwise a new client is created from the agent config.
    pub fn new(
        provider: &AgentProvider,
        mut config: AgentConfig,
        http_client: Option<HttpClient>,
    ) -> Result<Self> {
        config
            .preamble
            .get_or_insert_with(|| NER_SYSTEM_PROMPT.into());
        let mut builder = BaseAgent::builder(provider, config);
        if let Some(client) = http_client {
            builder = builder.http_client(client);
        }
        let base = builder.build().map_err(crate::error::convert)?;
        Ok(Self {
            base,
            state: Mutex::new(Vec::new()),
        })
    }

    /// Unique identifier for this agent instance (UUIDv7).
    pub fn id(&self) -> Uuid {
        self.base.id()
    }

    /// Access the usage tracker for this agent's LLM calls.
    pub fn tracker(&self) -> &UsageTracker {
        self.base.tracker()
    }

    /// The model name used by this agent.
    pub fn model_name(&self) -> &str {
        self.base.model_name()
    }

    /// Detect entities in text using structured output with text-based fallback.
    ///
    /// When [`NerContext::known_entities`] is non-empty the LLM is
    /// instructed to reuse their `entity_id` values for coreferent
    /// mentions, enabling cross-chunk coreference resolution.
    #[tracing::instrument(
        target = "nvisy_provider::agent::ner",
        skip_all,
        fields(text_len = ctx.text.len()),
    )]
    pub async fn detect(
        &self,
        ctx: &NerContext<'_>,
        config: &DetectionConfig,
    ) -> Result<Vec<NerEntity>> {
        let prompt = NerPromptBuilder::new(config, &ctx.known_entities).build(ctx.text);

        tracing::debug!(
            target: TARGET,
            prompt_len = prompt.len(),
            entity_kinds = config.entity_kinds.len(),
            known = ctx.known_entities.len(),
            "built ner prompt"
        );

        let result: NerEntities = self
            .base
            .prompt_structured(&prompt)
            .await
            .map_err(crate::error::convert)?;

        tracing::info!(
            target: TARGET,
            entity_count = result.entities.len(),
            "ner detection complete"
        );

        Ok(result.entities)
    }

    /// Detect entities in text, returning [`Entity`] values with
    /// [`TextLocation`] offsets.
    ///
    /// Manages coreference state internally: previously detected entities
    /// are carried forward so the LLM can assign consistent `entity_id`
    /// values across successive calls. Call [`reset`](Self::reset) to
    /// clear the state between documents.
    ///
    /// The caller is responsible for attaching span-level metadata
    /// (`span_index`, parent source) after this call.
    #[tracing::instrument(
        target = "nvisy_provider::agent::ner",
        skip_all,
        fields(text_len = text.len()),
    )]
    pub async fn detect_entities(
        &self,
        text: &str,
        config: &DetectionConfig,
    ) -> Result<Vec<Entity>> {
        let known = self.state.lock().await.clone();
        let ctx = NerContext::with_known(text, known);

        let ner_entities = self.detect(&ctx, config).await?;
        let model_info = ModelInfo::new(self.model_name(), ModelKind::Gateway);
        let mut entities = Vec::new();

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
            if confidence < config.confidence_threshold {
                continue;
            }

            let loc = if let Some(offsets) = ne.resolve_offsets(&ctx) {
                TextLocation {
                    start_offset: offsets.start,
                    end_offset: offsets.end,
                    ..Default::default()
                }
            } else {
                TextLocation::default()
            };

            let entity = Entity::builder()
                .with_category(category)
                .with_entity_kind(entity_kind)
                .with_value(&ne.value)
                .with_recognition_methods(vec![RecognitionMethod::ner(model_info.clone())])
                .with_confidence(confidence)
                .with_location(loc.into())
                .build()
                .expect("required fields provided");
            entities.push(entity);
        }

        // Update coreference state for the next call.
        let mut state = self.state.lock().await;
        let mut merge_ctx = NerContext::with_known(text, std::mem::take(&mut *state));
        merge_ctx.merge(ner_entities);
        *state = merge_ctx.known_entities;

        Ok(entities)
    }

    /// Clear coreference state. Call between documents.
    pub async fn reset(&self) {
        self.state.lock().await.clear();
    }
}
