//! Async retry execution for [`RetryPolicy`].

use std::future::Future;
use std::time::Duration;

use nvisy_core::Error;
use nvisy_ontology::workflow::{BackoffStrategy, RetryPolicy};

/// Async retry execution for [`RetryPolicy`].
pub(crate) trait RetryExt {
    /// Base delay as a [`Duration`].
    fn base_delay(&self) -> Duration;

    /// Computes the sleep duration for a given zero-based attempt number.
    fn compute_delay(&self, attempt: u32) -> Duration;

    /// Executes a fallible async closure with automatic retry.
    fn with_retry<F, Fut, T: Send>(&self, f: F) -> impl Future<Output = Result<T, Error>> + Send
    where
        F: FnMut() -> Fut + Send,
        Fut: Future<Output = Result<T, Error>> + Send;
}

impl RetryExt for RetryPolicy {
    fn base_delay(&self) -> Duration {
        Duration::from_millis(self.delay_ms)
    }

    fn compute_delay(&self, attempt: u32) -> Duration {
        let base = self.base_delay();
        match self.backoff {
            BackoffStrategy::Fixed => base,
            BackoffStrategy::Exponential => base * 2u32.saturating_pow(attempt),
            BackoffStrategy::Jitter => {
                let exp = base * 2u32.saturating_pow(attempt);
                let jitter_range = exp.as_millis() as u64 + 1;
                let jitter = Duration::from_millis(rand::random_range(0..jitter_range));
                exp + jitter
            }
            _ => base,
        }
    }

    async fn with_retry<F, Fut, T: Send>(&self, mut f: F) -> Result<T, Error>
    where
        F: FnMut() -> Fut + Send,
        Fut: Future<Output = Result<T, Error>> + Send,
    {
        let mut last_err = None;
        for attempt in 0..=self.max_retries {
            match f().await {
                Ok(v) => return Ok(v),
                Err(e) => {
                    if !e.is_retryable() || attempt == self.max_retries {
                        return Err(e);
                    }
                    last_err = Some(e);
                    let delay = self.compute_delay(attempt);
                    tokio::time::sleep(delay).await;
                }
            }
        }
        Err(last_err.unwrap_or_else(|| Error::runtime("Retry exhausted", "policy", false)))
    }
}
