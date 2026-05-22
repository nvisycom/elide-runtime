//! Async timeout execution for [`TimeoutPolicy`].

use std::future::Future;
use std::time::Duration;

use nvisy_core::Error;

use crate::workflow::TimeoutPolicy;

/// Async timeout execution for [`TimeoutPolicy`].
pub(crate) trait TimeoutExt {
    /// Wraps a future with a deadline.
    fn with_timeout<F, T: Send>(&self, f: F) -> impl Future<Output = Result<T, Error>> + Send
    where
        F: Future<Output = Result<T, Error>> + Send;
}

impl TimeoutExt for TimeoutPolicy {
    async fn with_timeout<F, T: Send>(&self, f: F) -> Result<T, Error>
    where
        F: Future<Output = Result<T, Error>> + Send,
    {
        match tokio::time::timeout(Duration::from_millis(self.duration_ms), f).await {
            Ok(result) => result,
            Err(_) => Err(Error::timeout(format!(
                "Operation timed out after {}ms",
                self.duration_ms,
            ))),
        }
    }
}
