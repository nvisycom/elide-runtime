//! Foundation agent that wraps provider-specific rig-core agents.

use std::borrow::Cow;

use reqwest_middleware::ClientWithMiddleware;
use rig::agent::Agent;
use rig::completion::Completion;
#[cfg(feature = "anthropic-claude")]
use rig::providers::anthropic;
#[cfg(feature = "google-gemini")]
use rig::providers::gemini;
use rig::providers::ollama;
#[cfg(feature = "openai-gpt")]
use rig::providers::openai;
use schemars::JsonSchema;
use serde::Serialize;
use serde::de::DeserializeOwned;
use uuid::Uuid;

use super::{AgentProvider, BaseAgentBuilder, ContextWindow, ResponseParser};
use crate::backend::UsageTracker;
use crate::error::Error;

/// Sampling, retry, context-window, and preamble settings shared by all agents.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Sampling temperature (default: 0.1).
    pub temperature: f64,
    /// Maximum output tokens (default: 4096).
    pub max_tokens: u64,
    /// Maximum retries for transient HTTP errors (default: 3).
    pub max_retries: u32,
    /// Context window for chunking large inputs.
    pub context_window: Option<ContextWindow>,
    /// System prompt (preamble) for the agent.
    pub preamble: Option<String>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            temperature: 0.1,
            max_tokens: 4096,
            max_retries: 3,
            context_window: None,
            preamble: None,
        }
    }
}

pub(crate) enum Agents {
    #[cfg(feature = "openai-gpt")]
    OpenAi(Agent<openai::completion::CompletionModel<ClientWithMiddleware>>),
    #[cfg(feature = "anthropic-claude")]
    Anthropic(Agent<anthropic::completion::CompletionModel<ClientWithMiddleware>>),
    #[cfg(feature = "google-gemini")]
    Gemini(Agent<gemini::completion::CompletionModel<ClientWithMiddleware>>),
    Ollama(Agent<ollama::CompletionModel<ClientWithMiddleware>>),
}

macro_rules! dispatch {
    ($inner:expr, |$agent:ident| $body:expr) => {
        match $inner {
            #[cfg(feature = "openai-gpt")]
            Agents::OpenAi($agent) => $body,
            #[cfg(feature = "anthropic-claude")]
            Agents::Anthropic($agent) => $body,
            #[cfg(feature = "google-gemini")]
            Agents::Gemini($agent) => $body,
            Agents::Ollama($agent) => $body,
        }
    };
}

/// Internal foundation agent wrapping a provider-specific rig-core agent
/// with usage tracking and structured-output fallback.
///
/// Specialized agents ([`NerAgent`], [`CvAgent`], [`OcrAgent`]) compose this
/// type rather than inheriting from it.
///
/// [`NerAgent`]: crate::NerAgent
/// [`CvAgent`]: crate::CvAgent
/// [`OcrAgent`]: crate::OcrAgent
pub(crate) struct BaseAgent {
    pub(super) id: Uuid,
    pub(super) inner: Agents,
    pub(super) context_window: Option<ContextWindow>,
    pub(super) tracker: UsageTracker,
}

impl BaseAgent {
    pub fn builder(provider: &AgentProvider, config: AgentConfig) -> BaseAgentBuilder {
        BaseAgentBuilder::new(provider, config)
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn tracker(&self) -> &UsageTracker {
        &self.tracker
    }

    /// If a context window is configured and `prompt` exceeds the input
    /// budget, summarize it to fit. Otherwise return the prompt unchanged.
    async fn maybe_compact<'a>(&self, prompt: &'a str) -> Result<Cow<'a, str>, Error> {
        let budget = match &self.context_window {
            Some(cw) => cw.input_budget(),
            None => return Ok(Cow::Borrowed(prompt)),
        };

        // Rough token estimate: 1 token ≈ 4 characters.
        if prompt.len() / 4 <= budget {
            return Ok(Cow::Borrowed(prompt));
        }

        tracing::info!(
            prompt_len = prompt.len(),
            budget,
            "prompt exceeds input budget, compacting"
        );

        let compact_prompt = format!(
            "Summarize the following text so it fits within {budget} tokens. \
             Preserve all key facts and details.\n\n{prompt}"
        );
        self.prompt_text_raw(&compact_prompt).await.map(Cow::Owned)
    }

    /// Plain-text completion without compaction (used internally by
    /// [`maybe_compact`] to avoid recursion).
    async fn prompt_text_raw(&self, prompt: &str) -> Result<String, Error> {
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

    /// Plain-text completion with usage tracking.
    ///
    /// Automatically compacts the prompt if a context window is configured
    /// and the prompt exceeds the input budget.
    #[tracing::instrument(skip_all, fields(agent_id = %self.id, mode = "text"))]
    pub async fn prompt_text(&self, prompt: &str) -> Result<String, Error> {
        let prompt = self.maybe_compact(prompt).await?;
        self.prompt_text_raw(&prompt).await
    }

    /// Structured-output prompt with usage tracking and JSON fallback.
    ///
    /// Automatically compacts the prompt if a context window is configured
    /// and the prompt exceeds the input budget.
    ///
    /// Sends a completion request with an `output_schema` so the provider
    /// constrains its response to valid JSON matching `T`. On deserialization
    /// failure the raw text is re-parsed via [`ResponseParser`].
    #[tracing::instrument(skip_all, fields(agent_id = %self.id, mode = "structured"))]
    pub async fn prompt_structured<T>(&self, prompt: &str) -> Result<T, Error>
    where
        T: DeserializeOwned + Default + JsonSchema + Serialize + Send + Sync,
    {
        let prompt = self.maybe_compact(prompt).await?;
        let schema = schemars::schema_for!(T);

        let (text, usage) = dispatch!(&self.inner, |agent| {
            let builder = agent
                .completion(&*prompt, vec![])
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
}
