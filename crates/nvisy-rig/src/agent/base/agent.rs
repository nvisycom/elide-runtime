//! [`BaseAgent`] — internal foundation agent wrapping rig-core's `Agent<M>`.

use rig::agent::Agent;
use rig::completion::{Completion, CompletionModel, Prompt, TypedPrompt};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::Serialize;

use nvisy_core::Error;

use crate::backend::{from_completion, from_prompt, UsageTracker};
use crate::bridge::ResponseParser;

use super::{BaseAgentBuilder, BaseAgentConfig};
use super::context::ContextWindow;

/// Internal foundation agent wrapping rig-core's [`Agent<M>`].
///
/// All prompt methods route through the built `Agent<M>`, which already
/// carries the preamble, temperature, max-tokens, and tools configured
/// via [`BaseAgentBuilder`].
///
/// Not exported — specialized agents (e.g. `NerAgent`) compose this.
pub(crate) struct BaseAgent<M: CompletionModel> {
    pub(super) agent: Agent<M>,
    pub(super) context_window: Option<ContextWindow>,
    pub(super) tracker: UsageTracker,
}

impl<M: CompletionModel> BaseAgent<M> {
    /// Create a new builder.
    pub fn builder(model: M, config: BaseAgentConfig) -> BaseAgentBuilder<M> {
        BaseAgentBuilder::new(model, config)
    }

    /// Access the usage tracker.
    pub fn tracker(&self) -> &UsageTracker {
        &self.tracker
    }

    /// Structured output prompt: tries `prompt_typed`, falls back to text +
    /// `parse_json`.
    #[tracing::instrument(skip_all, fields(mode = "structured"))]
    pub async fn prompt_structured<T>(&self, prompt: &str) -> Result<T, Error>
    where
        T: DeserializeOwned + Default + JsonSchema + Serialize + Send + Sync,
    {
        let structured_result: Result<T, _> = self.agent.prompt_typed::<T>(prompt).await;

        match structured_result {
            Ok(value) => {
                tracing::debug!("structured output succeeded");
                Ok(value)
            }
            Err(structured_err) => {
                tracing::warn!(
                    error = %structured_err,
                    "structured output failed, falling back to text-based parsing"
                );
                self.prompt_text_and_parse(prompt).await
            }
        }
    }

    /// Text completion through the agent, records usage.
    #[tracing::instrument(skip_all, fields(mode = "text"))]
    pub async fn prompt_text(&self, prompt: &str) -> Result<String, Error> {
        let builder = self
            .agent
            .completion(prompt, vec![])
            .await
            .map_err(from_completion)?;

        let response = builder.send().await.map_err(from_completion)?;
        let parsed = ResponseParser::extract_text(&response)?;
        self.tracker.record(&response.usage, 0);
        Ok(parsed.as_str().to_owned())
    }

    /// Plain text completion through the agent (no usage tracking).
    ///
    /// Uses `Prompt::prompt` which handles tool calls automatically but
    /// returns only the final text, not the raw response.
    #[tracing::instrument(skip_all, fields(mode = "prompt"))]
    pub async fn prompt(&self, prompt: &str) -> Result<String, Error> {
        self.agent.prompt(prompt).await.map_err(from_prompt)
    }

    /// Splits text via [`ContextWindow`], runs `prompt_structured` per chunk,
    /// and flattens results.
    #[tracing::instrument(skip_all, fields(mode = "chunked"))]
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

    /// Text-based fallback: complete → extract text → parse JSON.
    async fn prompt_text_and_parse<T>(&self, prompt: &str) -> Result<T, Error>
    where
        T: DeserializeOwned + Default,
    {
        let text = self.prompt_text(prompt).await?;
        ResponseParser::from_text(text.as_str()).parse_json()
    }
}
