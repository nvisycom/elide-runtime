//! Error mapping from rig-core errors to nvisy-core errors.

use rig::completion::CompletionError;

use nvisy_core::Error;

/// Convert a rig-core [`CompletionError`] into a [`nvisy_core::Error`].
pub fn from_completion(err: CompletionError) -> Error {
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
