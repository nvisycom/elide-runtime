//! [`BaseAgent`]: internal foundation agent wrapping rig-core agents.

use rig::completion::{Completion, Prompt};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

use crate::backend::UsageTracker;
use crate::bridge::ResponseParser;
use crate::error::Error;

use super::dispatch::{Agents, dispatch};
use super::{BaseAgentBuilder, BaseAgentConfig};
use super::context::ContextWindow;

/// Internal foundation agent wrapping a provider-specific rig-core agent.
///
/// All prompt methods dispatch to the concrete agent variant held inside
/// [`Agents`]. Specialized agents (e.g. `NerAgent`) compose this type.
///
/// Not exported: specialized agents (e.g. `NerAgent`) compose this.
pub(crate) struct BaseAgent {
    pub(super) id: Uuid,
    pub(super) inner: Agents,
    pub(super) context_window: Option<ContextWindow>,
    pub(super) tracker: UsageTracker,
}

impl BaseAgent {
    /// Create a new builder.
    pub fn builder(provider: &crate::agent::Provider, model_name: &str, config: BaseAgentConfig) -> BaseAgentBuilder {
        BaseAgentBuilder::new(provider, model_name, config)
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

        let (text, usage) = dispatch!(&self.inner, |agent| {
            let builder = agent
                .completion(prompt, vec![])
                .await
                .map_err(Error::from)?
                .output_schema(schema);

            let response = builder.send().await.map_err(Error::from)?;
            let parsed = ResponseParser::extract_text(&response)?;
            Ok::<_, Error>((parsed.into_string(), response.usage))
        })?;

        self.tracker.record(&usage, 0);

        let parser = ResponseParser::from_text(&text);
        match serde_json::from_str::<T>(&text) {
            Ok(value) => {
                tracing::debug!("structured output succeeded");
                Ok(value)
            }
            Err(structured_err) => {
                tracing::warn!(
                    error = %structured_err,
                    "structured JSON parse failed, falling back to text-based parsing"
                );
                parser.parse_json()
            }
        }
    }

    /// Text completion through the agent, records usage.
    #[tracing::instrument(skip_all, fields(agent_id = %self.id, mode = "text"))]
    pub async fn prompt_text(&self, prompt: &str) -> Result<String, Error> {
        let (text, usage) = dispatch!(&self.inner, |agent| {
            let builder = agent
                .completion(prompt, vec![])
                .await
                .map_err(Error::from)?;

            let response = builder.send().await.map_err(Error::from)?;
            let parsed = ResponseParser::extract_text(&response)?;
            Ok::<_, Error>((parsed.into_string(), response.usage))
        })?;

        self.tracker.record(&usage, 0);
        Ok(text)
    }

    /// Plain text completion through the agent (no usage tracking).
    ///
    /// Uses `Prompt::prompt` which handles tool calls automatically but
    /// returns only the final text, not the raw response.
    #[tracing::instrument(skip_all, fields(agent_id = %self.id, mode = "prompt"))]
    pub async fn prompt(&self, prompt: &str) -> Result<String, Error> {
        dispatch!(&self.inner, |agent| {
            agent.prompt(prompt).await.map_err(Error::from)
        })
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
