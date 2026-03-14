//! User-facing retry and timeout policy configuration types.
//!
//! These types are fields on [`GraphNode`](super::GraphNode) and are
//! serializable via serde. They carry no async or tokio dependencies.

mod retry;
mod timeout;

pub use self::retry::{BackoffStrategy, RetryPolicy};
pub use self::timeout::{TimeoutBehavior, TimeoutPolicy};
