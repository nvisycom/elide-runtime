//! Unified error type covering LLM provider, serialization, and tool failures.

use rig::audio_generation::AudioGenerationError;
use rig::completion::{CompletionError, PromptError, StructuredOutputError};
use rig::transcription::TranscriptionError;

/// Error type for all LLM interactions.
///
/// Use [`is_retryable`](Self::is_retryable) to decide whether a failed
/// request should be retried.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An HTTP / network error from the LLM provider.
    #[error("HTTP error: {0}")]
    Http(String),

    /// A JSON (de)serialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// The LLM provider returned an error response.
    #[error("Provider error: {0}")]
    Provider(String),

    /// The LLM response was malformed or unexpected.
    #[error("Response error: {0}")]
    Response(String),

    /// A request construction or validation error.
    #[error("Request error: {0}")]
    Request(String),

    /// A runtime error (tool failure, agent limits, generation errors, etc.).
    #[error("{0}")]
    Runtime(String),

    /// Wraps `nvisy_core::Error` from provider implementations.
    #[error(transparent)]
    Core(#[from] nvisy_core::Error),
}

impl Error {
    /// Whether this error is likely transient and safe to retry.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Http(_) => true,
            Self::Provider(msg) => is_retryable_provider_error(msg),
            _ => false,
        }
    }
}

impl From<CompletionError> for Error {
    fn from(err: CompletionError) -> Self {
        match err {
            CompletionError::HttpError(e) => Self::Http(e.to_string()),
            CompletionError::JsonError(e) => Self::Json(e),
            CompletionError::ProviderError(msg) => Self::Provider(msg),
            CompletionError::ResponseError(msg) => Self::Response(msg),
            CompletionError::RequestError(e) => Self::Request(e.to_string()),
            CompletionError::UrlError(e) => Self::Request(format!("URL: {e}")),
        }
    }
}

impl From<PromptError> for Error {
    fn from(err: PromptError) -> Self {
        match err {
            PromptError::CompletionError(e) => Self::from(e),
            PromptError::ToolError(e) => Self::Runtime(format!("tool: {e}")),
            PromptError::ToolServerError(e) => Self::Runtime(format!("tool server: {e}")),
            PromptError::MaxTurnsError { max_turns, .. } => {
                Self::Runtime(format!("agent exceeded max turn limit ({max_turns})"))
            }
            PromptError::PromptCancelled { reason, .. } => {
                Self::Runtime(format!("prompt cancelled: {reason}"))
            }
        }
    }
}

impl From<StructuredOutputError> for Error {
    fn from(err: StructuredOutputError) -> Self {
        match err {
            StructuredOutputError::PromptError(e) => Self::from(e),
            StructuredOutputError::DeserializationError(e) => {
                Self::Response(format!("structured output: {e}"))
            }
            StructuredOutputError::EmptyResponse => {
                Self::Response("model returned no content".to_string())
            }
        }
    }
}

impl From<TranscriptionError> for Error {
    fn from(err: TranscriptionError) -> Self {
        match err {
            TranscriptionError::HttpError(e) => Self::Http(e.to_string()),
            TranscriptionError::JsonError(e) => Self::Json(e),
            TranscriptionError::ProviderError(msg) => Self::Provider(msg),
            TranscriptionError::ResponseError(msg) => Self::Response(msg),
            TranscriptionError::RequestError(e) => Self::Request(e.to_string()),
            _ => Self::Runtime(err.to_string()),
        }
    }
}

impl From<AudioGenerationError> for Error {
    fn from(err: AudioGenerationError) -> Self {
        use rig::audio_generation::AudioGenerationError;
        match err {
            AudioGenerationError::HttpError(e) => Self::Http(e.to_string()),
            AudioGenerationError::JsonError(e) => Self::Json(e),
            AudioGenerationError::ProviderError(msg) => Self::Provider(msg),
            AudioGenerationError::ResponseError(msg) => Self::Response(msg),
            AudioGenerationError::RequestError(e) => Self::Request(e.to_string()),
        }
    }
}

impl From<Error> for nvisy_core::Error {
    fn from(err: Error) -> Self {
        if matches!(&err, Error::Core(_)) {
            return match err {
                Error::Core(inner) => inner,
                _ => unreachable!(),
            };
        }

        match &err {
            Error::Http(_) => nvisy_core::Error::connection(err.to_string(), "rig", true),
            Error::Json(_) => {
                nvisy_core::Error::new(nvisy_core::ErrorKind::Serialization, err.to_string())
                    .with_component("rig")
            }
            Error::Provider(msg) => {
                let retryable = is_retryable_provider_error(msg);
                nvisy_core::Error::connection(err.to_string(), "rig", retryable)
            }
            Error::Response(_) => nvisy_core::Error::runtime(err.to_string(), "rig", false),
            Error::Request(_) => nvisy_core::Error::validation(err.to_string(), "rig"),
            Error::Runtime(_) => nvisy_core::Error::runtime(err.to_string(), "rig", false),
            Error::Core(_) => unreachable!(),
        }
    }
}

/// Check if a provider error message indicates a retryable condition.
fn is_retryable_provider_error(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    lower.contains("rate_limit")
        || lower.contains("rate limit")
        || lower.contains("overloaded")
        || lower.contains("timeout")
        || lower.contains("429")
        || lower.contains("503")
        || lower.contains("529")
}
