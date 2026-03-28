//! Named Entity Recognition (NER) agent for textual PII/entity detection.
//!
//! [`NerAgent`] wraps a [`BaseAgent`](crate::backend::BaseAgent) with
//! NER-specific prompts. It is a pure LLM agent (no tools) that analyses
//! text and returns structured entity detections.

mod context;
mod output;
mod prompt;

use nvisy_core::Result;
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
}

impl NerAgent {
    /// Create a new NER agent.
    pub fn new(provider: &AgentProvider, mut config: AgentConfig) -> Result<Self> {
        config
            .preamble
            .get_or_insert_with(|| NER_SYSTEM_PROMPT.into());
        let base = BaseAgent::builder(provider, config)
            .build()
            .map_err(crate::error::convert)?;
        Ok(Self { base })
    }

    /// Create a new NER agent using a pre-built HTTP client.
    pub fn with_http_client(
        provider: &AgentProvider,
        mut config: AgentConfig,
        client: HttpClient,
    ) -> Result<Self> {
        config
            .preamble
            .get_or_insert_with(|| NER_SYSTEM_PROMPT.into());
        let base = BaseAgent::builder(provider, config)
            .http_client(client)
            .build()
            .map_err(crate::error::convert)?;
        Ok(Self { base })
    }

    /// Unique identifier for this agent instance (UUIDv7).
    pub fn id(&self) -> Uuid {
        self.base.id()
    }

    /// Access the usage tracker for this agent's LLM calls.
    pub fn tracker(&self) -> &UsageTracker {
        self.base.tracker()
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
}
