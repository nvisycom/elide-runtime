//! Text generation agent for generating synthetic replacement values.
//!
//! [`GenAgent`] wraps a [`BaseAgent`](super::BaseAgent) with
//! generation-specific prompts. It is a pure LLM agent (no tools) that
//! generates realistic fake values to replace detected PII/entities.

mod output;
mod prompt;

use nvisy_ontology::entity::EntityKind;
pub use output::{GenOutput, GeneratedEntity};
use prompt::{GEN_SYSTEM_PROMPT, GenPromptBuilder};
use uuid::Uuid;

use super::{AgentConfig, AgentProvider, BaseAgent};
use crate::backend::UsageTracker;
use crate::error::Error;

/// A request to generate a replacement value for a single entity.
#[derive(Debug, Clone)]
pub struct GenRequest {
    /// The type of entity to generate.
    pub entity_type: EntityKind,
    /// The original (real) value to replace.
    pub original_value: String,
    /// Optional surrounding text for context.
    pub context: Option<String>,
    /// Optional locale hint (e.g. `"en-US"`).
    pub locale: Option<String>,
}

/// Agent for generating synthetic replacement values using an LLM.
///
/// # Workflow
///
/// 1. Caller passes a batch of [`GenRequest`]s to
///    [`generate`](Self::generate).
/// 2. The agent builds a user prompt via [`GenPromptBuilder`].
/// 3. Structured output is parsed into `Vec<GeneratedEntity>`.
pub struct GenAgent {
    base: BaseAgent,
}

impl GenAgent {
    /// Create a new generation agent.
    pub fn new(provider: &AgentProvider, mut config: AgentConfig) -> Result<Self, Error> {
        config
            .preamble
            .get_or_insert_with(|| GEN_SYSTEM_PROMPT.into());
        let base = BaseAgent::builder(provider, config).build()?;
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

    /// Generate synthetic replacement values for a batch of entities.
    #[tracing::instrument(
        skip_all,
        fields(batch_size = requests.len(), agent = "gen"),
    )]
    pub async fn generate(&self, requests: &[GenRequest]) -> Result<Vec<GeneratedEntity>, Error> {
        let prompt = GenPromptBuilder::build(requests);

        tracing::debug!(prompt_len = prompt.len(), "built gen prompt");

        let result: GenOutput = self.base.prompt_structured(&prompt).await?;

        tracing::info!(
            entity_count = result.entities.len(),
            "text generation complete"
        );

        Ok(result.entities)
    }

    /// Generate a single synthetic replacement value.
    ///
    /// Uses plain-text completion instead of structured output, which is
    /// lighter-weight for a single value.
    #[tracing::instrument(
        skip_all,
        fields(entity_type = %request.entity_type, agent = "gen"),
    )]
    pub async fn generate_one(&self, request: &GenRequest) -> Result<GeneratedEntity, Error> {
        let prompt = GenPromptBuilder::build_one(request);
        let synthetic_value = self.base.prompt_text(&prompt).await?;

        Ok(GeneratedEntity {
            entity_type: request.entity_type,
            original_value: request.original_value.clone(),
            synthetic_value: synthetic_value.trim().to_owned(),
        })
    }
}
