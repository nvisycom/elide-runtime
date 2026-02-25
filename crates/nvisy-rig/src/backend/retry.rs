//! Tower retry policy with exponential backoff.

use std::{pin::Pin, time::Duration};

use nvisy_core::Error;
use tower::retry::Policy;

/// Tower retry policy with exponential backoff for retryable errors.
///
/// Generic over any request/response types: the request must be `Clone`
/// (so Tower can re-issue it) and the error type is [`nvisy_core::Error`]
/// whose `is_retryable()` flag drives the retry decision.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retries (default: 3).
    pub max_retries: u32,
    /// Initial backoff duration (default: 300ms).
    pub initial_backoff: Duration,
    /// Multiplicative backoff factor (default: 2.0).
    pub backoff_factor: f64,
    /// Maximum backoff duration cap (default: 5s).
    pub max_backoff: Duration,
    /// Current attempt counter (internal).
    attempts: u32,
    /// Current backoff (internal).
    current_backoff: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl RetryPolicy {
    /// Create a retry policy with default settings.
    pub fn new() -> Self {
        Self {
            max_retries: 3,
            initial_backoff: Duration::from_millis(300),
            backoff_factor: 2.0,
            max_backoff: Duration::from_secs(5),
            attempts: 0,
            current_backoff: Duration::from_millis(300),
        }
    }

    pub fn max_retries(&self) -> u32 {
        self.max_retries
    }
}

impl<Req, Res> Policy<Req, Res, Error> for RetryPolicy
where
    Req: Clone,
{
    type Future = Pin<Box<dyn std::future::Future<Output = ()> + Send>>;

    fn retry(&mut self, _req: &mut Req, result: &mut Result<Res, Error>) -> Option<Self::Future> {
        match result {
            Ok(_) => None,
            Err(err) => {
                if !err.is_retryable() || self.attempts >= self.max_retries {
                    return None;
                }

                self.attempts += 1;
                let backoff = self.current_backoff;

                tracing::warn!(
                    attempt = self.attempts,
                    max_retries = self.max_retries,
                    backoff_ms = backoff.as_millis() as u64,
                    error = %err,
                    "retrying after transient error"
                );

                self.current_backoff = Duration::from_secs_f64(
                    (self.current_backoff.as_secs_f64() * self.backoff_factor)
                        .min(self.max_backoff.as_secs_f64()),
                );

                Some(Box::pin(async move {
                    tokio::time::sleep(backoff).await;
                }))
            }
        }
    }

    fn clone_request(&mut self, req: &Req) -> Option<Req> {
        Some(req.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{DetectionConfig, DetectionRequest, DetectionResponse};
    use tower::retry::Policy;

    #[tokio::test]
    async fn retries_on_retryable_error() {
        let mut policy = RetryPolicy::new();
        let mut req = DetectionRequest {
            text: "test".into(),
            config: DetectionConfig {
                entity_kinds: vec![],
                confidence_threshold: 0.5,
                system_prompt: None,
            },
        };
        let mut result: Result<DetectionResponse, Error> =
            Err(Error::connection("transient", "test", true));

        let fut = policy.retry(&mut req, &mut result);
        assert!(fut.is_some());
    }

    #[tokio::test]
    async fn does_not_retry_non_retryable() {
        let mut policy = RetryPolicy::new();
        let mut req = DetectionRequest {
            text: "test".into(),
            config: DetectionConfig {
                entity_kinds: vec![],
                confidence_threshold: 0.5,
                system_prompt: None,
            },
        };
        let mut result: Result<DetectionResponse, Error> =
            Err(Error::validation("bad input", "test"));

        let fut = policy.retry(&mut req, &mut result);
        assert!(fut.is_none());
    }

    #[tokio::test]
    async fn does_not_retry_success() {
        let mut policy = RetryPolicy::new();
        let mut req = DetectionRequest {
            text: "test".into(),
            config: DetectionConfig {
                entity_kinds: vec![],
                confidence_threshold: 0.5,
                system_prompt: None,
            },
        };
        let mut result: Result<DetectionResponse, Error> = Ok(DetectionResponse {
            entities: vec![],
            usage: None,
        });

        let fut = policy.retry(&mut req, &mut result);
        assert!(fut.is_none());
    }
}
