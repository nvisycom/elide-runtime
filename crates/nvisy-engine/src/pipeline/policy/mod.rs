//! Async execution helpers for graph policy types.
//!
//! Adds `with_retry`, `with_timeout`, and `call` methods to
//! [`RetryPolicy`] and [`TimeoutPolicy`] via impl blocks that
//! use tokio for async sleep/timeout.
//!
//! [`RetryPolicy`]: crate::graph::RetryPolicy
//! [`TimeoutPolicy`]: crate::graph::TimeoutPolicy

mod retry;
mod timeout;
