//! Internal foundation agent wrapping rig-core's `Agent<M>`.

use std::sync::Arc;

use rig::agent::{Agent, AgentBuilder};
use rig::completion::{CompletionModel, TypedPrompt};
use rig::tool::{Tool, ToolDyn};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::Serialize;

use nvisy_core::Error;

use crate::backend::{from_completion, UsageTracker};
use crate::bridge::ResponseParser;

use super::context::ContextWindow;

/// Configuration for a [`BaseAgent`].
#[derive(Debug, Clone)]
pub struct BaseAgentConfig {
    /// Sampling temperature (default: 0.1).
    pub temperature: f64,
    /// Maximum output tokens (default: 4096).
    pub max_tokens: u64,
    /// Optional context window for chunking large inputs.
    pub context_window: Option<ContextWindow>,
}

impl Default for BaseAgentConfig {
    fn default() -> Self {
        Self {
            temperature: 0.1,
            max_tokens: 4096,
            context_window: None,
        }
    }
}

/// Internal foundation agent wrapping rig-core's [`Agent<M>`].
///
/// Not exported — specialized agents (e.g. `NerAgent`) compose this.
pub(crate) struct BaseAgent<M: CompletionModel> {
    agent: Agent<M>,
    model: Arc<M>,
    config: BaseAgentConfig,
    tracker: UsageTracker,
}

/// Builder for [`BaseAgent`] that handles rig-core's typestate for tools.
pub(crate) struct BaseAgentBuilder<M: CompletionModel> {
    model: Arc<M>,
    config: BaseAgentConfig,
    preamble: Option<String>,
    tools: Vec<Box<dyn ToolDyn>>,
}

impl<M: CompletionModel> BaseAgentBuilder<M> {
    /// Create a new builder with the given model and config.
    pub fn new(model: M, config: BaseAgentConfig) -> Self {
        Self {
            model: Arc::new(model),
            config,
            preamble: None,
            tools: Vec::new(),
        }
    }

    /// Set the system prompt (preamble).
    pub fn preamble(mut self, preamble: impl Into<String>) -> Self {
        self.preamble = Some(preamble.into());
        self
    }

    /// Add a tool to the agent.
    pub fn tool(mut self, tool: impl Tool + 'static) -> Self {
        self.tools.push(Box::new(tool));
        self
    }

    /// Build the [`BaseAgent`].
    pub fn build(self) -> BaseAgent<M> {
        let agent = if self.tools.is_empty() {
            let mut builder = AgentBuilder::new((*self.model).clone())
                .temperature(self.config.temperature)
                .max_tokens(self.config.max_tokens);

            if let Some(ref preamble) = self.preamble {
                builder = builder.preamble(preamble);
            }

            builder.build()
        } else {
            let mut builder = AgentBuilder::new((*self.model).clone())
                .temperature(self.config.temperature)
                .max_tokens(self.config.max_tokens)
                .tools(self.tools);

            if let Some(ref preamble) = self.preamble {
                builder = builder.preamble(preamble);
            }

            builder.build()
        };

        BaseAgent {
            agent,
            model: self.model,
            config: self.config,
            tracker: UsageTracker::new(),
        }
    }
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

    /// Access the config.
    pub fn config(&self) -> &BaseAgentConfig {
        &self.config
    }

    /// Structured output prompt: tries `prompt_typed`, falls back to text +
    /// `parse_json`.
    #[tracing::instrument(skip_all, fields(mode = "structured"))]
    pub async fn prompt_structured<T>(&self, prompt: &str, system: Option<&str>) -> Result<T, Error>
    where
        T: DeserializeOwned + Default + JsonSchema + Serialize + Send + Sync,
    {
        // Try structured output first.
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
                self.prompt_text_and_parse(prompt, system).await
            }
        }
    }

    /// Raw text completion, records usage.
    #[tracing::instrument(skip_all, fields(mode = "text"))]
    pub async fn prompt_text(&self, prompt: &str, system: Option<&str>) -> Result<String, Error> {
        let mut builder = self
            .model
            .completion_request(prompt)
            .temperature(self.config.temperature)
            .max_tokens(self.config.max_tokens);

        if let Some(preamble) = system {
            builder = builder.preamble(preamble.to_string());
        }

        let response = builder.send().await.map_err(from_completion)?;
        let parsed = ResponseParser::extract_text(&response)?;
        self.tracker.record(&response.usage, 0);
        Ok(parsed.as_str().to_owned())
    }

    /// Splits text via [`ContextWindow`], runs `prompt_structured` per chunk,
    /// and flattens results.
    #[tracing::instrument(skip_all, fields(mode = "chunked"))]
    pub async fn prompt_chunked<T, F>(
        &self,
        text: &str,
        build_prompt: F,
        system: Option<&str>,
    ) -> Result<Vec<T>, Error>
    where
        T: DeserializeOwned + Default + JsonSchema + Serialize + Send + Sync,
        F: Fn(&str) -> String,
        Vec<T>: Default,
    {
        let chunks = match &self.config.context_window {
            Some(cw) => cw.split_to_fit(text),
            None => vec![text],
        };

        let mut all_results = Vec::new();
        for chunk in chunks {
            let prompt = build_prompt(chunk);
            let chunk_results: Vec<T> = self.prompt_structured(&prompt, system).await?;
            all_results.extend(chunk_results);
        }

        Ok(all_results)
    }

    /// Text-based fallback: complete → extract text → parse JSON.
    async fn prompt_text_and_parse<T>(&self, prompt: &str, system: Option<&str>) -> Result<T, Error>
    where
        T: DeserializeOwned + Default,
    {
        let text = self.prompt_text(prompt, system).await?;
        ResponseParser::from_text(text.as_str()).parse_json()
    }
}
