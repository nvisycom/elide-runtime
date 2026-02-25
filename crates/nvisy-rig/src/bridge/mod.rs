//! Core bridge between rig-core and the [`LlmBackend`] trait.

pub mod prompt;
pub mod response;

pub use prompt::PromptBuilder;
pub use response::{EntityParser, ResponseParser};

use std::sync::atomic::{AtomicU32, Ordering};

use serde_json::Value;

use rig::completion::CompletionModel;

use nvisy_core::Error;

use crate::backend::{LlmBackend, LlmConfig};
use crate::backend::ErrorMapper;
use crate::backend::UsageTracker;
use crate::backend::RetryPolicy;

/// Configuration for a [`RigBackend`].
#[derive(Debug, Clone)]
pub struct RigBackendConfig {
    /// Sampling temperature (default: 0.1).
    pub temperature: f64,
    /// Maximum output tokens (default: 4096).
    pub max_tokens: u64,
    /// Retry policy for transient errors.
    pub retry: RetryPolicy,
}

impl Default for RigBackendConfig {
    fn default() -> Self {
        Self {
            temperature: 0.1,
            max_tokens: 4096,
            retry: RetryPolicy::new(),
        }
    }
}

/// Production [`LlmBackend`] implementation wrapping a rig-core
/// [`CompletionModel`].
pub struct RigBackend<M> {
    model: M,
    config: RigBackendConfig,
    tracker: UsageTracker,
}

impl<M: CompletionModel> RigBackend<M> {
    /// Create a new backend with the given model and configuration.
    pub fn new(model: M, config: RigBackendConfig) -> Self {
        Self {
            model,
            config,
            tracker: UsageTracker::new(),
        }
    }

    /// Access the usage tracker for this backend.
    pub fn tracker(&self) -> &UsageTracker {
        &self.tracker
    }

    /// Send a single completion request to the model.
    async fn send_request(
        &self,
        user_prompt: &str,
        system_prompt: Option<&str>,
    ) -> Result<(String, rig::completion::Usage), Error> {
        let mut builder = self
            .model
            .completion_request(user_prompt)
            .temperature(self.config.temperature)
            .max_tokens(self.config.max_tokens);

        if let Some(preamble) = system_prompt {
            builder = builder.preamble(preamble.to_string());
        }

        let response = builder.send().await.map_err(ErrorMapper::from_completion)?;
        let text = ResponseParser::extract_text(&response)?;
        Ok((text, response.usage))
    }
}

#[async_trait::async_trait]
impl<M> LlmBackend for RigBackend<M>
where
    M: CompletionModel + Send + Sync + 'static,
{
    #[tracing::instrument(skip_all, fields(text_len = text.len()))]
    async fn detect_text(
        &self,
        text: &str,
        config: &LlmConfig,
    ) -> Result<Vec<Value>, Error> {
        let user_prompt = PromptBuilder::new(config).build(text);
        let system_prompt = config.system_prompt.as_deref();

        let call_count = AtomicU32::new(0);
        let result = self
            .config
            .retry
            .execute(|| {
                call_count.fetch_add(1, Ordering::Relaxed);
                self.send_request(&user_prompt, system_prompt)
            })
            .await;

        // Actual retries = total calls - 1 (the first attempt is not a retry).
        let actual_retries = call_count.load(Ordering::Relaxed).saturating_sub(1);

        match result {
            Ok((response_text, usage)) => {
                self.tracker.record(&usage, actual_retries);

                tracing::debug!(
                    input_tokens = usage.input_tokens,
                    output_tokens = usage.output_tokens,
                    retries = actual_retries,
                    "LLM request completed"
                );

                ResponseParser::parse_entities(&response_text)
            }
            Err(e) => Err(e),
        }
    }
}
