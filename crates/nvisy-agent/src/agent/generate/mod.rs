//! Text generation agent for generating synthetic replacement values.
//!
//! [`GenAgent`] wraps the crate-internal base agent with
//! generation-specific prompts. Pure LLM, no tools — generates
//! realistic fake values to replace detected PII/entities.

mod output;
mod prompt;

use nvisy_core::Result;
use nvisy_ontology::entity::EntityKind;
use uuid::Uuid;

pub use self::output::{GenOutput, GeneratedEntity};
use self::prompt::{GEN_SYSTEM_PROMPT, GenPromptBuilder};
use crate::agent::base::{BaseAgent, UsageTracker};
use crate::agent::{AgentConfig, AgentProvider};

const TARGET: &str = "nvisy_agent::agent::generate";

/// A request to generate a replacement value for a single entity.
#[derive(Debug, Clone)]
pub struct GenRequest {
    /// The type of entity to generate.
    pub entity_type: EntityKind,
    /// The original (real) value to replace.
    pub original_value: String,
    /// Optional surrounding text for context.
    pub context: Option<String>,
}

/// Agent for generating synthetic replacement values using an LLM.
///
/// # Workflow
///
/// 1. Caller passes a batch of [`GenRequest`]s to
///    [`generate`].
/// 2. The agent builds a user prompt via `GenPromptBuilder`.
/// 3. Structured output is parsed into `Vec<GeneratedEntity>`.
///
/// [`generate`]: Self::generate
pub struct GenAgent {
    base: BaseAgent,
}

impl GenAgent {
    /// Create a new generation agent. The HTTP client is built
    /// internally from `config.max_retries` and otherwise-default
    /// settings.
    pub fn new(provider: &AgentProvider, mut config: AgentConfig) -> Result<Self> {
        config
            .preamble
            .get_or_insert_with(|| GEN_SYSTEM_PROMPT.into());
        let base = BaseAgent::builder(provider, config)
            .build()
            .map_err(crate::error::convert)?;
        Ok(Self { base })
    }

    /// Unique identifier for this agent instance (UUIDv7).
    pub fn id(&self) -> Uuid {
        self.base.id()
    }

    /// Configured model name.
    pub fn model_name(&self) -> &str {
        self.base.model_name()
    }

    /// Access the usage tracker for this agent's LLM calls.
    pub fn tracker(&self) -> &UsageTracker {
        self.base.tracker()
    }

    /// Generate synthetic replacement values for a batch of entities.
    #[tracing::instrument(
        target = "nvisy_agent::agent::generate",
        skip_all,
        fields(batch_size = requests.len()),
    )]
    pub async fn generate(&self, requests: &[GenRequest]) -> Result<Vec<GeneratedEntity>> {
        let prompt = GenPromptBuilder::build(requests);

        tracing::debug!(target: TARGET, prompt_len = prompt.len(), "built gen prompt");

        let result: GenOutput = self
            .base
            .prompt_structured(&prompt)
            .await
            .map_err(crate::error::convert)?;

        tracing::info!(
            target: TARGET,
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
        target = "nvisy_agent::agent::generate",
        skip_all,
        fields(entity_type = %request.entity_type),
    )]
    pub async fn generate_one(&self, request: &GenRequest) -> Result<GeneratedEntity> {
        let prompt = GenPromptBuilder::build_one(request);
        let synthetic_value = self
            .base
            .prompt_text(&prompt)
            .await
            .map_err(crate::error::convert)?;

        Ok(GeneratedEntity {
            entity_type: request.entity_type,
            original_value: request.original_value.clone(),
            synthetic_value: synthetic_value.trim().to_owned(),
        })
    }
}
