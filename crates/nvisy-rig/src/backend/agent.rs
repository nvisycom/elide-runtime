//! Foundation agent that wraps provider-specific rig-core agents.

#[path = "builder.rs"]
mod builder;

pub(crate) use builder::BaseAgentBuilder;

use reqwest_middleware::ClientWithMiddleware;
use rig::agent::Agent;
use rig::completion::{Completion, Prompt};
use rig::providers::{anthropic, gemini, ollama, openai};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

use super::context::ContextWindow;
use super::provider::Provider;
use super::UsageTracker;
use crate::bridge::ResponseParser;
use crate::error::Error;

/// Sampling, retry, and context-window settings shared by all agents.
#[derive(Debug, Clone)]
pub struct BaseAgentConfig {
    /// Sampling temperature (default: 0.1).
    pub temperature: f64,
    /// Maximum output tokens (default: 4096).
    pub max_tokens: u64,
    /// Maximum retries for transient HTTP errors (default: 3).
    pub max_retries: u32,
    /// Context window for chunking large inputs.
    pub context_window: Option<ContextWindow>,
}

impl Default for BaseAgentConfig {
    fn default() -> Self {
        Self {
            temperature: 0.1,
            max_tokens: 4096,
            max_retries: 3,
            context_window: None,
        }
    }
}

enum Agents {
    OpenAi(Agent<openai::completion::CompletionModel<ClientWithMiddleware>>),
    Anthropic(Agent<anthropic::completion::CompletionModel<ClientWithMiddleware>>),
    Gemini(Agent<gemini::completion::CompletionModel<ClientWithMiddleware>>),
    Ollama(Agent<ollama::CompletionModel<ClientWithMiddleware>>),
}

macro_rules! dispatch {
    ($inner:expr, |$agent:ident| $body:expr) => {
        match $inner {
            Agents::OpenAi($agent) => $body,
            Agents::Anthropic($agent) => $body,
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
    id: Uuid,
    inner: Agents,
    context_window: Option<ContextWindow>,
    tracker: UsageTracker,
}

impl BaseAgent {
    pub fn builder(provider: &Provider, config: BaseAgentConfig) -> BaseAgentBuilder {
        BaseAgentBuilder::new(provider, config)
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn tracker(&self) -> &UsageTracker {
        &self.tracker
    }

    /// Structured-output prompt with usage tracking and JSON fallback.
    ///
    /// Sends a completion request with an `output_schema` so the provider
    /// constrains its response to valid JSON matching `T`. On deserialization
    /// failure the raw text is re-parsed via [`ResponseParser`].
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

    /// Text completion with usage tracking.
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

    /// Plain text completion (no usage tracking).
    #[tracing::instrument(skip_all, fields(agent_id = %self.id, mode = "prompt"))]
    pub async fn prompt(&self, prompt: &str) -> Result<String, Error> {
        dispatch!(&self.inner, |agent| {
            agent.prompt(prompt).await.map_err(Error::from)
        })
    }

    /// Summarize text to fit within the context window's input budget.
    ///
    /// Returns the text unchanged when no context window is configured or
    /// the text already fits.
    #[tracing::instrument(skip_all, fields(agent_id = %self.id, mode = "compact"))]
    pub async fn prompt_compact(&self, text: &str) -> Result<String, Error> {
        let cw = match &self.context_window {
            Some(cw) if !cw.fits(text) => cw,
            _ => return Ok(text.to_owned()),
        };

        let budget = cw.input_budget();
        let prompt = format!(
            "Summarize the following text to fit within {budget} tokens. \
             Preserve all key entities, names, numbers, dates, and facts. \
             Remove redundancy and filler. Return ONLY the condensed text, \
             no preamble.\n\n{text}"
        );

        self.prompt_text(&prompt).await
    }

    /// Split text via [`ContextWindow`], run `prompt_structured` per chunk,
    /// and flatten results.
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
