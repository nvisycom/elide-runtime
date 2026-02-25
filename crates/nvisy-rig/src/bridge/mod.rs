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

/// Configuration for [`ServiceBackend`] (and its [`RigBackend`] specialisation).
#[derive(Debug, Clone)]
pub struct RigBackendConfig {
    /// Retry policy for transient errors.
    pub retry: RetryPolicy,
}

impl Default for RigBackendConfig {
    fn default() -> Self {
        Self {
            retry: RetryPolicy::new(),
        }
    }
}

/// Generic Tower service adapter.
///
/// Wraps any inner service `S` with a retry policy and usage tracking.
/// The inner service handles prompt construction and LLM interaction;
/// the wrapper provides observability and resilience.
pub struct ServiceBackend<S> {
    inner: S,
    config: RigBackendConfig,
    tracker: Arc<UsageTracker>,
}

impl<S> ServiceBackend<S> {
    /// Create a new service backend wrapping an arbitrary inner service.
    pub fn new(inner: S, config: RigBackendConfig) -> Self {
        Self {
            inner,
            config,
            tracker: Arc::new(UsageTracker::new()),
        }
    }

    /// Access the retry policy.
    pub fn retry_policy(&self) -> &RetryPolicy {
        &self.config.retry
    }

    /// Access the usage tracker for this backend.
    pub fn tracker(&self) -> &UsageTracker {
        &self.tracker
    }
}

impl<S> tower::Service<DetectionRequest> for ServiceBackend<S>
where
    S: tower::Service<DetectionRequest, Response = DetectionResponse, Error = Error>,
    S::Future: Send + 'static,
{
    type Response = DetectionResponse;
    type Error = Error;
    type Future = std::pin::Pin<Box<dyn std::future::Future<Output = Result<DetectionResponse, Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: DetectionRequest) -> Self::Future {
        let tracker = Arc::clone(&self.tracker);
        let fut = self.inner.call(req);

        Box::pin(async move {
            let span = tracing::info_span!("service_backend_call");
            let _enter = span.enter();

            let response = fut.await?;

            if let Some(ref usage) = response.usage {
                tracker.record(usage, 0);

                tracing::debug!(
                    input_tokens = usage.input_tokens,
                    output_tokens = usage.output_tokens,
                    "LLM request completed"
                );
            }

            Ok(response)
        })
    }
}

/// Inner service that drives a raw rig-core [`CompletionModel`].
///
/// This is the low-level service that constructs prompts and parses
/// responses. Wrap it in [`ServiceBackend`] for retry and usage tracking.
pub struct RigBackendInner<M> {
    model: Arc<M>,
    temperature: f64,
    max_tokens: u64,
}

impl<M> tower::Service<DetectionRequest> for RigBackendInner<M>
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
        let temperature = self.temperature;
        let max_tokens = self.max_tokens;

        Box::pin(async move {
            let span = tracing::info_span!("rig_backend_call");
            let _enter = span.enter();

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

            Ok(DetectionResponse {
                entities,
                usage: Some(response.usage),
            })
        })
    }
}

/// Production detection service wrapping a rig-core [`CompletionModel`].
///
/// This is a convenience alias for `ServiceBackend<RigBackendInner<M>>`.
/// Use [`RigBackend::from_model`] to construct one.
pub type RigBackend<M> = ServiceBackend<RigBackendInner<M>>;

impl<M: CompletionModel> RigBackend<M> {
    /// Create a new backend with the given model and configuration.
    ///
    /// Temperature and max_tokens are configured on the inner model service.
    /// The [`RigBackendConfig`] controls retry policy.
    pub fn from_model(model: M, temperature: f64, max_tokens: u64, config: RigBackendConfig) -> Self {
        let inner = RigBackendInner {
            model: Arc::new(model),
            temperature,
            max_tokens,
        };
        ServiceBackend::new(inner, config)
    }
}
