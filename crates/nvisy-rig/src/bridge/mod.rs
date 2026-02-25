//! Core bridge between rig-core and the Tower-based detection service.

mod prompt;
mod response;

pub use prompt::PromptBuilder;
pub use response::{EntityParser, ResponseParser};

use std::sync::Arc;
use std::task::{Context, Poll};

use rig::client::CompletionClient;
use rig::completion::CompletionModel;
use rig::providers::{anthropic, gemini, ollama, openai};

use crate::agent::Provider;
use crate::agent::base::provider::ProviderClient;
use crate::backend::{DetectionRequest, DetectionResponse, RetryPolicy, UsageTracker};
use crate::error::Error;

/// Configuration for [`ServiceBackend`] (and its [`RigBackend`] specialisation).
#[derive(Debug, Clone, Default)]
pub struct RigBackendConfig {
    /// Retry policy for transient errors.
    pub retry: RetryPolicy,
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
    S: tower::Service<DetectionRequest, Response = DetectionResponse, Error = nvisy_core::Error>,
    S::Future: Send + 'static,
{
    type Response = DetectionResponse;
    type Error = nvisy_core::Error;
    type Future = std::pin::Pin<Box<dyn std::future::Future<Output = Result<DetectionResponse, nvisy_core::Error>> + Send>>;

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

enum InnerModel {
    OpenAi(Arc<openai::completion::CompletionModel>),
    Anthropic(Arc<anthropic::completion::CompletionModel>),
    Gemini(Arc<gemini::completion::CompletionModel>),
    Ollama(Arc<ollama::CompletionModel>),
}

impl InnerModel {
    fn clone_arc(&self) -> Self {
        match self {
            Self::OpenAi(m) => Self::OpenAi(Arc::clone(m)),
            Self::Anthropic(m) => Self::Anthropic(Arc::clone(m)),
            Self::Gemini(m) => Self::Gemini(Arc::clone(m)),
            Self::Ollama(m) => Self::Ollama(Arc::clone(m)),
        }
    }
}

macro_rules! dispatch_model {
    ($inner:expr, |$model:ident| $body:expr) => {
        match $inner {
            InnerModel::OpenAi($model) => $body,
            InnerModel::Anthropic($model) => $body,
            InnerModel::Gemini($model) => $body,
            InnerModel::Ollama($model) => $body,
        }
    };
}

/// Inner service that drives a rig-core completion model.
///
/// This is the low-level service that constructs prompts and parses
/// responses. Wrap it in [`ServiceBackend`] for retry and usage tracking.
pub struct RigBackendInner {
    model: InnerModel,
    temperature: f64,
    max_tokens: u64,
}

impl tower::Service<DetectionRequest> for RigBackendInner {
    type Response = DetectionResponse;
    type Error = nvisy_core::Error;
    type Future = std::pin::Pin<Box<dyn std::future::Future<Output = Result<DetectionResponse, nvisy_core::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: DetectionRequest) -> Self::Future {
        let user_prompt = PromptBuilder::new(&req.config).build(&req.text);
        let system_prompt = req.config.system_prompt.clone();
        let model = self.model.clone_arc();
        let temperature = self.temperature;
        let max_tokens = self.max_tokens;

        Box::pin(async move {
            let span = tracing::info_span!("rig_backend_call");
            let _enter = span.enter();

            let (parsed, usage) = dispatch_model!(&model, |model| {
                let mut builder = model
                    .completion_request(&user_prompt)
                    .temperature(temperature)
                    .max_tokens(max_tokens);

                if let Some(ref preamble) = system_prompt {
                    builder = builder.preamble(preamble.clone());
                }

                let response = builder.send().await.map_err(|e| {
                    nvisy_core::Error::from(Error::from(e))
                })?;
                let text = ResponseParser::extract_text(&response)
                    .map_err(nvisy_core::Error::from)?;
                Ok::<_, nvisy_core::Error>((text, response.usage))
            })?;

            let entities = parsed.parse_json().map_err(nvisy_core::Error::from)?;

            Ok(DetectionResponse {
                entities,
                usage: Some(usage),
            })
        })
    }
}

/// Production detection service wrapping a rig-core completion model.
///
/// This is a convenience alias for `ServiceBackend<RigBackendInner>`.
/// Use [`RigBackend::from_provider`] to construct one.
pub type RigBackend = ServiceBackend<RigBackendInner>;

impl RigBackend {
    /// Create a new backend from a provider, model name, and configuration.
    pub fn from_provider(
        provider: &Provider,
        model_name: &str,
        temperature: f64,
        max_tokens: u64,
        config: RigBackendConfig,
    ) -> Result<Self, Error> {
        let client = ProviderClient::from_provider(provider)?;
        let model = match client {
            ProviderClient::OpenAi(c) => InnerModel::OpenAi(Arc::new(c.completion_model(model_name))),
            ProviderClient::Anthropic(c) => InnerModel::Anthropic(Arc::new(c.completion_model(model_name))),
            ProviderClient::Gemini(c) => InnerModel::Gemini(Arc::new(c.completion_model(model_name))),
            ProviderClient::Ollama(c) => InnerModel::Ollama(Arc::new(c.completion_model(model_name))),
        };

        let inner = RigBackendInner {
            model,
            temperature,
            max_tokens,
        };

        Ok(ServiceBackend::new(inner, config))
    }
}
