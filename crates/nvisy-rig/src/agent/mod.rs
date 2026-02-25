//! Structured output backend using rig-core's JSON schema enforcement.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use std::sync::Arc;

use rig::agent::{Agent, AgentBuilder};
use rig::completion::{CompletionModel, TypedPrompt};

use nvisy_core::Error;

use crate::backend::{LlmBackend, LlmConfig};
use crate::bridge::prompt::PromptBuilder;
use crate::bridge::response::ResponseParser;
use crate::bridge::RigBackendConfig;
use crate::backend::ErrorMapper;
use crate::backend::UsageTracker;

/// A list of entities returned by structured output.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct EntityList {
    /// Detected entities.
    pub entities: Vec<RawEntity>,
}

/// A single raw entity from structured LLM output.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct RawEntity {
    /// Broad classification (e.g. "pii", "phi", "financial", "credentials").
    pub category: String,
    /// Specific entity type (e.g. "email_address", "person_name").
    pub entity_type: String,
    /// The matched text value.
    pub value: String,
    /// Detection confidence (0.0 -- 1.0).
    pub confidence: f64,
    /// Start byte offset in the input text.
    pub start_offset: usize,
    /// End byte offset in the input text.
    pub end_offset: usize,
}

impl RawEntity {
    /// Convert this raw entity into a [`serde_json::Value`] dict.
    pub fn into_value(self) -> Value {
        serde_json::json!({
            "category": self.category,
            "entity_type": self.entity_type,
            "value": self.value,
            "confidence": self.confidence,
            "start_offset": self.start_offset,
            "end_offset": self.end_offset,
        })
    }
}

/// Backend that uses rig-core's structured output (JSON schema enforcement)
/// for entity detection.
///
/// Falls back to text-based parsing if structured output fails.
pub struct StructuredBackend<M: CompletionModel> {
    agent: Agent<M>,
    model: Arc<M>,
    config: RigBackendConfig,
    tracker: UsageTracker,
}

impl<M: CompletionModel> StructuredBackend<M> {
    /// Create a new structured backend.
    pub fn new(model: M, config: RigBackendConfig) -> Self {
        let model = Arc::new(model);
        let agent = AgentBuilder::new((*model).clone())
            .temperature(config.temperature)
            .max_tokens(config.max_tokens)
            .build();

        Self {
            agent,
            model,
            config,
            tracker: UsageTracker::new(),
        }
    }

    /// Access the usage tracker for this backend.
    pub fn tracker(&self) -> &UsageTracker {
        &self.tracker
    }
}

#[async_trait::async_trait]
impl<M> LlmBackend for StructuredBackend<M>
where
    M: CompletionModel + Send + Sync + 'static,
{
    #[tracing::instrument(skip_all, fields(text_len = text.len(), mode = "structured"))]
    async fn detect_text(
        &self,
        text: &str,
        config: &LlmConfig,
    ) -> Result<Vec<Value>, Error> {
        let user_prompt = PromptBuilder::new(config).build(text);

        // Try structured output first.
        let structured_result: Result<EntityList, _> = self
            .agent
            .prompt_typed::<EntityList>(&user_prompt)
            .await;

        match structured_result {
            Ok(entity_list) => {
                tracing::debug!(
                    count = entity_list.entities.len(),
                    "structured output succeeded"
                );
                Ok(entity_list.entities.into_iter().map(RawEntity::into_value).collect())
            }
            Err(structured_err) => {
                tracing::warn!(
                    error = %structured_err,
                    "structured output failed, falling back to text-based parsing"
                );

                // Fall back to text-based completion using the model directly.
                let mut builder = self
                    .model
                    .completion_request(&user_prompt)
                    .temperature(self.config.temperature)
                    .max_tokens(self.config.max_tokens);

                if let Some(ref preamble) = config.system_prompt {
                    builder = builder.preamble(preamble.clone());
                }

                let response = builder.send().await.map_err(ErrorMapper::from_completion)?;
                let response_text = ResponseParser::extract_text(&response)?;
                self.tracker.record(&response.usage, 0);
                ResponseParser::parse_entities(&response_text)
            }
        }
    }
}
