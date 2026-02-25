//! Error mapping from rig-core errors to nvisy-core errors.

use rig::completion::{CompletionError, PromptError};

use nvisy_core::Error;

/// Convert a rig-core [`PromptError`] into a [`nvisy_core::Error`].
pub(crate) fn from_prompt(err: PromptError) -> Error {
    match err {
        PromptError::CompletionError(e) => from_completion(e),
        PromptError::ToolError(e) => {
            Error::runtime(format!("Tool error: {e}"), "rig", false)
        }
        PromptError::ToolServerError(e) => {
            Error::runtime(format!("Tool server error: {e}"), "rig", true)
        }
        PromptError::MaxTurnsError { max_turns, .. } => {
            Error::runtime(
                format!("Agent exceeded max turn limit ({max_turns})"),
                "rig",
                false,
            )
        }
        PromptError::PromptCancelled { reason, .. } => {
            Error::runtime(format!("Prompt cancelled: {reason}"), "rig", false)
        }
    }
}

/// Convert a rig-core [`CompletionError`] into a [`nvisy_core::Error`].
pub(crate) fn from_completion(err: CompletionError) -> Error {
    match err {
        CompletionError::HttpError(e) => {
            Error::connection(format!("HTTP error: {e}"), "rig", true)
        }
        CompletionError::JsonError(e) => {
            Error::new(nvisy_core::ErrorKind::Serialization, format!("JSON error: {e}"))
                .with_component("rig")
        }
        CompletionError::ProviderError(msg) => {
            let retryable = is_retryable_provider_error(&msg);
            Error::connection(format!("Provider error: {msg}"), "rig", retryable)
        }
        CompletionError::ResponseError(msg) => {
            Error::runtime(format!("Response error: {msg}"), "rig", false)
        }
        CompletionError::RequestError(e) => {
            Error::validation(format!("Request error: {e}"), "rig")
        }
        CompletionError::UrlError(e) => {
            Error::validation(format!("URL error: {e}"), "rig")
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
