//! [`BaseAgent`]: internal foundation agent wrapping rig-core's `Agent<M>`.

use rig::agent::Agent;
use rig::completion::{Completion, CompletionModel, Prompt};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

use nvisy_core::Error;

use crate::backend::UsageTracker;
use crate::error::Error as RigError;
use crate::bridge::ResponseParser;

use super::{BaseAgentBuilder, BaseAgentConfig};
use super::context::ContextWindow;

/// Internal foundation agent wrapping rig-core's [`Agent<M>`].
///
/// All prompt methods route through the built `Agent<M>`, which already
/// carries the preamble, temperature, max_tokens, and tools configured
/// via [`BaseAgentBuilder`].
///
/// Not exported: specialized agents (e.g. `NerAgent`) compose this.
pub(crate) struct BaseAgent<M: CompletionModel> {
    pub(super) id: Uuid,
    pub(super) agent: Agent<M>,
    pub(super) context_window: Option<ContextWindow>,
    pub(super) tracker: UsageTracker,
}

impl<M: CompletionModel> BaseAgent<M> {
    /// Create a new builder.
    pub fn builder(model: M, config: BaseAgentConfig) -> BaseAgentBuilder<M> {
        BaseAgentBuilder::new(model, config)
    }

    /// Unique identifier for this agent instance (UUIDv7).
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Access the usage tracker.
    pub fn tracker(&self) -> &UsageTracker {
        &self.tracker
    }

    /// Structured output prompt with usage tracking.
    ///
    /// Uses `agent.completion()` with an `output_schema` so the provider
    /// constrains its response to valid JSON matching `T`. Falls back to
    /// text-based parsing on deserialization failure.
    #[tracing::instrument(skip_all, fields(agent_id = %self.id, mode = "structured"))]
    pub async fn prompt_structured<T>(&self, prompt: &str) -> Result<T, Error>
    where
        T: DeserializeOwned + Default + JsonSchema + Serialize + Send + Sync,
    {
        let schema = schemars::schema_for!(T);

        let builder = self
            .agent
            .completion(prompt, vec![])
            .await
            .map_err(|e| Error::from(RigError::from(e)))?
            .output_schema(schema);

        let response = builder.send().await.map_err(|e| Error::from(RigError::from(e)))?;
        let parsed = ResponseParser::extract_text(&response)?;
        self.tracker.record(&response.usage, 0);

        match serde_json::from_str::<T>(parsed.as_str()) {
            Ok(value) => {
                tracing::debug!("structured output succeeded");
                Ok(value)
            }
            Err(structured_err) => {
                tracing::warn!(
                    error = %structured_err,
                    "structured JSON parse failed, falling back to text-based parsing"
                );
                parsed.parse_json()
            }
        }
    }

    /// Text completion through the agent, records usage.
    #[tracing::instrument(skip_all, fields(agent_id = %self.id, mode = "text"))]
    pub async fn prompt_text(&self, prompt: &str) -> Result<String, Error> {
        let builder = self
            .agent
            .completion(prompt, vec![])
            .await
            .map_err(|e| Error::from(RigError::from(e)))?;

        let response = builder.send().await.map_err(|e| Error::from(RigError::from(e)))?;
        let parsed = ResponseParser::extract_text(&response)?;
        self.tracker.record(&response.usage, 0);
        Ok(parsed.as_str().to_owned())
    }

    /// Plain text completion through the agent (no usage tracking).
    ///
    /// Uses `Prompt::prompt` which handles tool calls automatically but
    /// returns only the final text, not the raw response.
    #[tracing::instrument(skip_all, fields(agent_id = %self.id, mode = "prompt"))]
    pub async fn prompt(&self, prompt: &str) -> Result<String, Error> {
        self.agent.prompt(prompt).await.map_err(|e| Error::from(RigError::from(e)))
    }

    /// Summarize text via LLM to fit within the context window's input budget.
    ///
    /// Delegates to [`ContextWindow::compact`]. Returns the text unchanged if
    /// no context window is configured or the text already fits.
    #[tracing::instrument(skip_all, fields(agent_id = %self.id, mode = "compact"))]
    pub async fn prompt_compact(&self, text: &str) -> Result<String, Error> {
        match &self.context_window {
            Some(cw) => cw.compact(text, self).await,
            None => Ok(text.to_owned()),
        }
    }

    /// Splits text via [`ContextWindow`], runs `prompt_structured` per chunk,
    /// and flattens results.
    #[tracing::instrument(skip_all, fields(agent_id = %self.id, mode = "chunked"))]
    pub async fn prompt_chunked<T, F>(
        &self,
        text: &str,
        build_prompt: F,
    ) -> Result<Vec<T>, Error>
    where
        T: DeserializeOwned + Default + JsonSchema + Serialize + Send + Sync,
        F: Fn(&str) -> String,
        Vec<T>: Default,
    {
        let chunks = match &self.context_window {
            Some(cw) => cw.split_to_fit(text),
            None => vec![text],
        };

        let mut all_results = Vec::new();
        for chunk in chunks {
            let prompt = build_prompt(chunk);
            let chunk_results: Vec<T> = self.prompt_structured(&prompt).await?;
            all_results.extend(chunk_results);
        }

        Ok(all_results)
    }

}
