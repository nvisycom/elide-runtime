//! Execution policy configuration types.
//!
//! These types live on [`GraphNode`] (retry, timeout) or [`Graph`]
//! (concurrency) and are serializable via serde.
//!
//! [`GraphNode`]: super::GraphNode
//! [`Graph`]: super::Graph

mod concurrency;
mod retry;
mod timeout;

pub use self::concurrency::ConcurrencyPolicy;
pub use self::retry::{BackoffStrategy, RetryPolicy};
pub use self::timeout::{TimeoutBehavior, TimeoutPolicy};
