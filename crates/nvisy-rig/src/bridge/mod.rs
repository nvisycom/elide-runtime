//! Core bridge between rig-core and the Tower-based detection service.

mod prompt;
mod response;

pub use prompt::PromptBuilder;
pub use response::{EntityParser, ResponseParser};

use std::sync::Arc;
use std::task::{Context, Poll};

use rig::completion::CompletionModel;

use nvisy_core::Error;

use crate::backend::{
    from_completion, DetectionRequest, DetectionResponse,
    RetryPolicy, UsageTracker,
};

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

/// Production detection service wrapping a rig-core [`CompletionModel`].
///
/// Implements `tower::Service<DetectionRequest>`.
pub struct RigBackend<M> {
    model: Arc<M>,
    config: RigBackendConfig,
    tracker: Arc<UsageTracker>,
}

impl<M: CompletionModel> RigBackend<M> {
    /// Create a new backend with the given model and configuration.
    pub fn new(model: M, config: RigBackendConfig) -> Self {
        Self {
            model: Arc::new(model),
            config,
            tracker: Arc::new(UsageTracker::new()),
        }
    }

    /// Access the usage tracker for this backend.
    pub fn tracker(&self) -> &UsageTracker {
        &self.tracker
    }
}

impl<M> tower::Service<DetectionRequest> for RigBackend<M>
where
    M: CompletionModel + Send + Sync + 'static,
{
    type Response = DetectionResponse;
    type Error = Error;
    type Future = std::pin::Pin<Box<dyn std::future::Future<Output = Result<DetectionResponse, Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: DetectionRequest) -> Self::Future {
        let user_prompt = PromptBuilder::new(&req.config).build(&req.text);
        let system_prompt = req.config.system_prompt.clone();
        let model = Arc::clone(&self.model);
        let temperature = self.config.temperature;
        let max_tokens = self.config.max_tokens;
        let tracker = Arc::clone(&self.tracker);

        Box::pin(async move {
            let mut builder = model
                .completion_request(&user_prompt)
                .temperature(temperature)
                .max_tokens(max_tokens);

            if let Some(ref preamble) = system_prompt {
                builder = builder.preamble(preamble.clone());
            }

            let response = builder.send().await.map_err(from_completion)?;
            let parsed = ResponseParser::extract_text(&response)?;
            let entities = parsed.parse_json()?;

            tracker.record(&response.usage, 0);

            tracing::debug!(
                input_tokens = response.usage.input_tokens,
                output_tokens = response.usage.output_tokens,
                "LLM request completed"
            );

            Ok(DetectionResponse {
                entities,
                usage: Some(response.usage),
            })
        })
    }
}
