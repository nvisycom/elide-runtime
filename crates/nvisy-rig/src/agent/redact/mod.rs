//! Redactor agent for context-aware semantic redaction.
//!
//! [`RedactorAgent`] is a pure LLM agent (no tools) that takes detected
//! entities and their surrounding text and recommends a
//! [`TextRedactionMethod`](nvisy_ontology::specification::TextRedactionMethod)
//! for each one. It considers sensitivity level, document context, and
//! downstream utility when choosing between masking, replacement, hashing,
//! synthesis, pseudonymisation, and removal.

mod output;
mod prompt;

pub use output::{RawRedaction, RedactorOutput};

use rig::completion::CompletionModel;
use uuid::Uuid;

use nvisy_core::Error;
use nvisy_ontology::specification::RedactorInput;

use crate::backend::UsageTracker;

use super::{BaseAgent, BaseAgentConfig};
use prompt::{REDACTOR_SYSTEM_PROMPT, RedactorPromptBuilder};

/// Agent for context-aware redaction recommendations.
///
/// # Workflow
///
/// 1. Caller passes source text and a slice of [`RedactorInput`] entities
///    to [`recommend`](Self::recommend).
/// 2. The agent serialises the entities as JSON and builds a user prompt
///    via [`RedactorPromptBuilder`].
/// 3. The LLM returns structured output mapping each entity to a
///    [`TextRedactionMethod`](nvisy_ontology::specification::TextRedactionMethod)
///    with a suggested replacement string.
/// 4. The result is parsed into `Vec<RawRedaction>`.
pub struct RedactorAgent<M: CompletionModel> {
    base: BaseAgent<M>,
}

impl<M: CompletionModel> RedactorAgent<M> {
    /// Create a new redactor agent with the given model and config.
    pub fn new(model: M, config: BaseAgentConfig) -> Self {
        let base = BaseAgent::builder(model, config)
            .preamble(REDACTOR_SYSTEM_PROMPT)
            .build();
        Self { base }
    }

    /// Unique identifier for this agent instance (UUIDv7).
    pub fn id(&self) -> Uuid {
        self.base.id()
    }

    /// Access the usage tracker for this agent's LLM calls.
    pub fn tracker(&self) -> &UsageTracker {
        self.base.tracker()
    }

    /// Recommend redaction methods for detected entities in the given text.
    #[tracing::instrument(
        skip_all,
        fields(text_len = text.len(), entity_count = entities.len(), agent = "redactor"),
    )]
    pub async fn recommend(
        &self,
        text: &str,
        entities: &[RedactorInput],
    ) -> Result<Vec<RawRedaction>, Error> {
        let prompt = RedactorPromptBuilder::build(text, entities)?;

        tracing::debug!(prompt_len = prompt.len(), "built redactor prompt");

        let result: RedactorOutput = self.base.prompt_structured(&prompt).await?;

        tracing::info!(
            redaction_count = result.redactions.len(),
            "redaction recommendations complete"
        );

        Ok(result.redactions)
    }
}
